//! Download worker: page fetching, copying and chapter manifests.

use tokio::io::AsyncWriteExt;

use crate::image::{HttpClient, ImageCache, fetch_image_inner, get_reader_chapter_inner};

use super::*;

async fn copy_atomic(source: &Path, destination: &Path) -> Result<u64> {
    static TEMP_ID: AtomicU64 = AtomicU64::new(0);

    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    let temporary = destination.with_file_name(format!(
        ".{name}.{}.{}.part",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));

    if let Err(error) = tokio::fs::copy(source, &temporary).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error.into());
    }
    if let Err(error) = tokio::fs::rename(&temporary, destination).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error.into());
    }
    Ok(tokio::fs::metadata(destination).await?.len())
}

async fn write_manifest_atomic(path: &Path, manifest: &DownloadManifest) -> Result<()> {
    static TEMP_ID: AtomicU64 = AtomicU64::new(0);

    let bytes = serde_json::to_vec(manifest)?;
    let temporary = path.with_file_name(format!(
        ".{MANIFEST_FILE_NAME}.{}.{}.part",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = async {
        let file = tokio::fs::File::create(&temporary).await?;
        let mut writer = tokio::io::BufWriter::new(file);
        writer.write_all(&bytes).await?;
        writer.flush().await?;
        writer.get_ref().sync_all().await?;
        Ok::<(), std::io::Error>(())
    }
    .await;

    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error.into());
    }
    if let Err(error) = tokio::fs::rename(&temporary, path).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error.into());
    }
    Ok(())
}

async fn existing_pages(directory: &Path) -> Result<HashMap<u32, String>> {
    let mut pages = HashMap::new();
    let mut entries = match tokio::fs::read_dir(directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(pages),
        Err(error) => return Err(error.into()),
    };

    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".part") {
            continue;
        }
        let Some((stem, extension)) = name.split_once('.') else {
            continue;
        };
        if extension.is_empty() || stem.len() != 5 {
            continue;
        }
        if let Ok(number) = stem.parse::<u32>()
            && number > 0
        {
            pages.insert(number, name);
        }
    }
    Ok(pages)
}

fn extension_of(path: &str) -> Result<&str> {
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric()))
        .ok_or_else(|| AppError::Unexpected("downloaded image has no usable extension".into()))
}

async fn ensure_cover(
    store: &DownloadsStore,
    cache: &ImageCache,
    app: &AppHandle,
    path: &Path,
    root: &Path,
    key: &DownloadKey,
    run_id: u64,
) -> Result<()> {
    let Some(record) = store.record_snapshot(path, key)? else {
        return Ok(());
    };
    let directory = work_dir(root, &record);
    if let Some(file) = record.cover_file.as_deref()
        && tokio::fs::metadata(directory.join(file)).await.is_ok()
    {
        return Ok(());
    }
    let Some(thumb) = record.thumb.clone() else {
        return Ok(());
    };

    let http = app.state::<HttpClient>();
    let cached =
        match fetch_image_inner(http.inner(), cache, thumb, Some(record.platform.clone())).await {
            Ok(image) => image,
            Err(error) => {
                logging::warn(
                    "download",
                    format!(
                        "failed to download the cover: platform={} id={} error={error}",
                        key.0, key.1
                    ),
                );
                return Ok(());
            }
        };
    if !store.is_current(path, key, run_id)? {
        return Ok(());
    }

    let extension = extension_of(&cached.path)?;
    let file_name = format!("cover.{extension}");
    if let Err(error) = copy_atomic(Path::new(&cached.path), &directory.join(&file_name)).await {
        logging::warn(
            "download",
            format!(
                "failed to store the cover: platform={} id={} error={error}",
                key.0, key.1
            ),
        );
        return Ok(());
    }
    store.set_cover(path, key, run_id, file_name)?;
    Ok(())
}

async fn download_work(app: &AppHandle, key: &DownloadKey, run_id: u64) -> Result<()> {
    let store = app.state::<DownloadsStore>();
    let cache = app.state::<ImageCache>();
    let path = downloads_index_path(app)?;
    let root = downloads_root(app)?;
    let Some(record) = store.record_snapshot(&path, key)? else {
        return Ok(());
    };

    let directory = work_dir(&root, &record);
    tokio::fs::create_dir_all(&directory).await?;
    ensure_cover(store.inner(), cache.inner(), app, &path, &root, key, run_id).await?;

    let http = app.state::<HttpClient>();
    let chapter_count = record.chapters.len() as u32;
    for (chapter_index, chapter) in record.chapters.iter().enumerate() {
        if chapter.completed {
            continue;
        }
        if !store.is_current(&path, key, run_id)? {
            return Ok(());
        }

        let chapter_number = chapter_index as u32 + 1;
        let source = match get_reader_chapter_inner(
            http.inner(),
            &record.platform,
            &record.id,
            chapter.id.as_deref(),
        )
        .await
        {
            Ok(source) => source,
            Err(error) => {
                logging::warn(
                    "download",
                    format!(
                        "failed to read chapter pages: platform={} id={} chapter={:?} error={error}",
                        key.0, key.1, chapter.id
                    ),
                );
                let failure = DownloadFailure::from_app(&error);
                store.fail_chapter(
                    &path,
                    key,
                    run_id,
                    chapter_index as u32,
                    chapter.page_count,
                    failure,
                )?;
                continue;
            }
        };
        if !store.is_current(&path, key, run_id)? {
            return Ok(());
        }
        if source.pages.is_empty() {
            let failure = DownloadFailure::from_app(&AppError::NotFound {
                resource: "chapter pages".into(),
            });
            store.fail_chapter(&path, key, run_id, chapter_index as u32, 0, failure)?;
            continue;
        }

        store.note_chapter_started(
            &path,
            key,
            run_id,
            chapter_index as u32,
            chapter_count,
            source.pages.len() as u32,
            source.chapter_title.clone(),
        )?;

        let chapter_directory = chapter_dir(&root, &record, chapter.id.as_deref());
        tokio::fs::create_dir_all(&chapter_directory).await?;
        let existing = existing_pages(&chapter_directory).await?;
        let mut manifest_pages = Vec::with_capacity(source.pages.len());
        let mut failure: Option<DownloadFailure> = None;

        for (page_index, page) in source.pages.iter().enumerate() {
            if !store.is_current(&path, key, run_id)? {
                return Ok(());
            }

            let page_number = page_index as u32 + 1;
            if let Some(file) = existing.get(&page_number) {
                let target = chapter_directory.join(file);
                if tokio::fs::metadata(&target)
                    .await
                    .is_ok_and(|metadata| metadata.is_file())
                {
                    manifest_pages.push(ManifestPage {
                        url: page.url.clone(),
                        file: file.clone(),
                        width: page.width,
                        height: page.height,
                    });
                    store.note_page(
                        &path,
                        key,
                        run_id,
                        page_number,
                        source.pages.len() as u32,
                        0,
                        false,
                    )?;
                    continue;
                }
            }

            let cached = match fetch_image_inner(
                http.inner(),
                cache.inner(),
                page.url.clone(),
                Some(record.platform.clone()),
            )
            .await
            {
                Ok(image) => image,
                Err(error) => {
                    failure = Some(DownloadFailure::from_app(&error));
                    break;
                }
            };
            if !store.is_current(&path, key, run_id)? {
                return Ok(());
            }

            let extension = match extension_of(&cached.path) {
                Ok(extension) => extension,
                Err(error) => {
                    failure = Some(DownloadFailure::from_app(&error));
                    break;
                }
            };
            let file = format!("{page_number:05}.{extension}");
            let bytes =
                match copy_atomic(Path::new(&cached.path), &chapter_directory.join(&file)).await {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        failure = Some(DownloadFailure::from_app(&error));
                        break;
                    }
                };
            if !store.note_page(
                &path,
                key,
                run_id,
                page_number,
                source.pages.len() as u32,
                bytes,
                true,
            )? {
                return Ok(());
            }
            manifest_pages.push(ManifestPage {
                url: page.url.clone(),
                file,
                width: page.width,
                height: page.height,
            });
        }

        if failure.is_none() && manifest_pages.len() == source.pages.len() {
            let manifest = DownloadManifest {
                version: MANIFEST_VERSION,
                chapter_id: chapter.id.clone(),
                chapter_title: source.chapter_title.clone(),
                pages: manifest_pages,
            };
            match write_manifest_atomic(&chapter_directory.join(MANIFEST_FILE_NAME), &manifest)
                .await
            {
                Ok(()) => {
                    store.complete_chapter(
                        &path,
                        key,
                        run_id,
                        chapter_index as u32,
                        source.pages.len() as u32,
                    )?;
                    logging::info(
                        "download",
                        format!(
                            "chapter downloaded: platform={} id={} chapter={}/{} pages={}",
                            key.0,
                            key.1,
                            chapter_number,
                            chapter_count,
                            source.pages.len()
                        ),
                    );
                }
                Err(error) => {
                    failure = Some(DownloadFailure::from_app(&error));
                }
            }
        }

        if let Some(failure) = failure {
            logging::warn(
                "download",
                format!(
                    "chapter failed: platform={} id={} chapter={} code={}",
                    key.0, key.1, chapter_number, failure.code
                ),
            );
            store.fail_chapter(
                &path,
                key,
                run_id,
                chapter_index as u32,
                source.pages.len() as u32,
                failure,
            )?;
        }
    }

    Ok(())
}

async fn run_download(app: AppHandle, key: DownloadKey, run_id: u64) {
    let store = app.state::<DownloadsStore>();
    let Ok(_permit) = store.queue.acquire().await else {
        return;
    };

    let path = match downloads_index_path(&app) {
        Ok(path) => path,
        Err(error) => {
            logging::error(
                "download",
                format!("failed to resolve the downloads index: {error}"),
            );
            return;
        }
    };

    match store.begin_worker(&path, &key, run_id) {
        Ok(true) => {}
        Ok(false) => return,
        Err(error) => {
            logging::error(
                "download",
                format!(
                    "failed to start download worker: platform={} id={} error={error}",
                    key.0, key.1
                ),
            );
            return;
        }
    }

    logging::info(
        "download",
        format!("download started: platform={} id={}", key.0, key.1),
    );

    let result = download_work(&app, &key, run_id).await;
    if let Err(error) = &result {
        logging::error(
            "download",
            format!(
                "download stopped: platform={} id={} error={error}",
                key.0, key.1
            ),
        );
    }
    store.finish_worker(&path, &key, run_id, result);

    if let Ok(mut progress) = store.lock_progress() {
        progress.remove(&key);
    }
}

pub(super) fn spawn_download(app: AppHandle, key: DownloadKey, run_id: u64) {
    tauri::async_runtime::spawn(async move {
        run_download(app, key, run_id).await;
    });
}
