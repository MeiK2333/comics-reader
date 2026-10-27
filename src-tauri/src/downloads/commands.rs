//! Tauri commands for the download index.

use super::worker::spawn_download;
use super::*;

/// Run index work on the blocking pool. The store uses `std::sync::Mutex` and
/// synchronous file IO, so it must not run on a tokio worker thread.
async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| AppError::Unexpected(format!("downloads task failed: {error}")))?
}

/// Start (or extend) a whole-work download.
#[tauri::command]
pub(crate) async fn start_download(
    app: AppHandle,
    input: DownloadInput,
) -> Result<DownloadSummary> {
    let task_app = app.clone();
    let (summary, run_id, key) = blocking(move || {
        let path = downloads_index_path(&task_app)?;
        let root = downloads_root(&task_app)?;
        let key = download_key(&input.platform, &input.id)?;
        let (summary, run_id) = task_app
            .state::<DownloadsStore>()
            .start(&path, &root, input)?;
        Ok((summary, run_id, key))
    })
    .await?;

    if let Some(run_id) = run_id {
        spawn_download(app, key, run_id);
    }
    Ok(summary)
}

/// Pause a queued or running download.
#[tauri::command]
pub(crate) async fn pause_download(
    app: AppHandle,
    platform: String,
    id: String,
) -> Result<DownloadSummary> {
    blocking(move || {
        let path = downloads_index_path(&app)?;
        let root = downloads_root(&app)?;
        app.state::<DownloadsStore>()
            .pause(&path, &root, &platform, &id)
    })
    .await
}

/// Resume an interrupted download from its stored chapter list.
#[tauri::command]
pub(crate) async fn resume_download(
    app: AppHandle,
    platform: String,
    id: String,
) -> Result<DownloadSummary> {
    let task_app = app.clone();
    let (summary, run_id, key) = blocking(move || {
        let path = downloads_index_path(&task_app)?;
        let root = downloads_root(&task_app)?;
        let key = download_key(&platform, &id)?;
        let (summary, run_id) = task_app
            .state::<DownloadsStore>()
            .resume(&path, &root, &platform, &id)?;
        Ok((summary, run_id, key))
    })
    .await?;

    if let Some(run_id) = run_id {
        spawn_download(app, key, run_id);
    }
    Ok(summary)
}

/// Delete one download and its files. Active work must be paused first.
#[tauri::command]
pub(crate) async fn delete_download(app: AppHandle, platform: String, id: String) -> Result<()> {
    blocking(move || {
        let path = downloads_index_path(&app)?;
        let root = downloads_root(&app)?;
        app.state::<DownloadsStore>()
            .delete(&path, &root, &platform, &id)
    })
    .await
}

/// Return one page of downloads, newest first.
#[tauri::command]
pub(crate) async fn list_downloads(
    app: AppHandle,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<DownloadPage> {
    blocking(move || {
        let path = downloads_index_path(&app)?;
        let root = downloads_root(&app)?;
        let (offset, limit) = page_window(offset, limit);
        app.state::<DownloadsStore>()
            .list(&path, &root, offset, limit)
    })
    .await
}

/// Look up one saved download, including its live progress if it is running.
#[tauri::command]
pub(crate) async fn get_download(
    app: AppHandle,
    platform: String,
    id: String,
) -> Result<Option<DownloadDetail>> {
    blocking(move || {
        let path = downloads_index_path(&app)?;
        let root = downloads_root(&app)?;
        app.state::<DownloadsStore>()
            .get(&path, &root, &platform, &id)
    })
    .await
}

/// Resolve a downloaded chapter into the same wire type the reader uses.
#[tauri::command]
pub(crate) async fn get_downloaded_chapter(
    app: AppHandle,
    platform: String,
    id: String,
    chapter_id: Option<String>,
) -> Result<Option<ReaderChapter>> {
    let single_chapter = crate::platforms::find_catalog(platform.trim())
        .is_some_and(|catalog| catalog.single_chapter());
    let chapter_id = if single_chapter {
        None
    } else {
        chapter_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let chapter_id = match chapter_id {
        Some(value) => Some(numeric_id(value, "chapterId")?),
        None => None,
    };

    blocking(move || {
        let path = downloads_index_path(&app)?;
        let root = downloads_root(&app)?;
        app.state::<DownloadsStore>().read_local_chapter(
            &path,
            &root,
            &platform,
            &id,
            chapter_id.as_deref(),
        )
    })
    .await
}
