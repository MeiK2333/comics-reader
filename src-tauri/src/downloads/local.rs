//! Resolve a downloaded chapter manifest into the reader wire type.

use std::io::ErrorKind;

use super::*;

pub(super) fn read_local_chapter(
    store: &DownloadsStore,
    path: &Path,
    root: &Path,
    platform: &str,
    id: &str,
    chapter_id: Option<&str>,
) -> Result<Option<ReaderChapter>> {
    let key = download_key(platform, id)?;
    let Some(record) = store.record_snapshot(path, &key)? else {
        return Ok(None);
    };

    let chapter_index = match chapter_id {
        Some(chapter_id) => record
            .chapters
            .iter()
            .position(|chapter| chapter.completed && chapter.id.as_deref() == Some(chapter_id)),
        None => {
            if record.chapters.len() == 1 && record.chapters[0].completed {
                Some(0)
            } else {
                None
            }
        }
    };
    let Some(chapter_index) = chapter_index else {
        return Ok(None);
    };
    let chapter = &record.chapters[chapter_index];
    let directory = chapter_dir(root, &record, chapter.id.as_deref());
    let manifest_path = directory.join(MANIFEST_FILE_NAME);

    let bytes = match fs::read(&manifest_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let manifest = match serde_json::from_slice::<DownloadManifest>(&bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            logging::warn(
                "download",
                format!(
                    "ignoring an unreadable download manifest at {}: {error}",
                    manifest_path.display()
                ),
            );
            return Ok(None);
        }
    };
    if manifest.version != MANIFEST_VERSION
        || manifest.chapter_id != chapter.id
        || manifest.pages.is_empty()
    {
        logging::warn(
            "download",
            format!(
                "ignoring a stale download manifest at {}",
                manifest_path.display()
            ),
        );
        return Ok(None);
    }

    let mut pages = Vec::with_capacity(manifest.pages.len());
    for page in manifest.pages {
        if !is_safe_file_name(&page.file) {
            logging::warn("download", "download manifest contains an unsafe file name");
            return Ok(None);
        }
        let file = directory.join(&page.file);
        match fs::metadata(&file) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => return Ok(None),
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        }
        pages.push(ReaderPage {
            url: page.url,
            width: page.width,
            height: page.height,
            local_path: Some(path_to_string(&file)?),
        });
    }

    let previous = record.chapters[..chapter_index]
        .iter()
        .rev()
        .find(|item| item.completed)
        .and_then(chapter_ref);
    let next = record.chapters[chapter_index + 1..]
        .iter()
        .find(|item| item.completed)
        .and_then(chapter_ref);

    Ok(Some(ReaderChapter {
        title: record.title.clone(),
        thumb: record.thumb.clone().unwrap_or_default(),
        chapter_id: chapter.id.clone(),
        chapter_title: chapter.title.clone(),
        pages,
        previous,
        next,
    }))
}
