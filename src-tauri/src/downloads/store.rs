//! Download index state machine and persistence.

use super::*;
use tokio::sync::SemaphorePermit;

#[derive(Default)]
pub(crate) struct DownloadsData {
    pub(super) loaded: bool,
    pub(super) entries: HashMap<DownloadKey, DownloadRecord>,
    pub(super) corrupt: bool,
    pub(super) sizes_backfilled: bool,
}

/// Sum every file that belongs to a work. Temporary `.part` files are ignored
/// so an interrupted copy cannot inflate the reported size.
fn work_dir_size(directory: &Path) -> Result<u64> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };

    let mut total = 0_u64;
    for entry in entries {
        let entry = entry?;
        if entry.file_name().to_string_lossy().ends_with(".part") {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if metadata.is_dir() {
            total = total.saturating_add(work_dir_size(&entry.path())?);
        } else if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        }
    }
    Ok(total)
}

/// Tauri-managed store. The index is loaded once and then kept in memory; the
/// semaphore serializes works and each run's id invalidates the previous
/// worker when the user pauses or resumes.
pub(crate) struct DownloadsStore {
    pub(super) data: Mutex<DownloadsData>,
    pub(super) queue: Semaphore,
    pub(super) progress: Mutex<HashMap<DownloadKey, DownloadProgress>>,
    /// Monotonic run ids. A deleted work can be queued again, so the counter
    /// must never restart from the record; that would let an old worker pass
    /// `is_current` for the new run.
    pub(super) run_seq: AtomicU64,
}

impl Default for DownloadsStore {
    fn default() -> Self {
        Self {
            data: Mutex::new(DownloadsData::default()),
            queue: Semaphore::new(1),
            progress: Mutex::new(HashMap::new()),
            run_seq: AtomicU64::new(0),
        }
    }
}

impl DownloadsStore {
    pub(super) fn next_run_id(&self) -> u64 {
        self.run_seq.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Hold the single download slot while cache cleanup runs.
    ///
    /// Downloads copy page images out of the image cache, so clearing it while
    /// a worker holds a page path can make that copy fail. A worker must own
    /// the slot for its whole run; holding it here also keeps a newly spawned
    /// worker from starting in the middle of the deletion. `None` means a
    /// download is queued or running.
    pub(crate) fn try_reserve_slot(&self) -> Option<SemaphorePermit<'_>> {
        self.queue.try_acquire().ok()
    }

    pub(super) fn with_data<T>(
        &self,
        path: &Path,
        operation: impl FnOnce(&mut DownloadsData, &Path) -> Result<T>,
    ) -> Result<T> {
        let mut data = self.lock_data()?;
        if !data.loaded {
            data.load(path)?;
            data.loaded = true;
        }
        operation(&mut data, path)
    }

    pub(super) fn lock_data(&self) -> Result<MutexGuard<'_, DownloadsData>> {
        self.data
            .lock()
            .map_err(|_| AppError::Unexpected("downloads lock was poisoned".into()))
    }

    pub(super) fn lock_progress(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<DownloadKey, DownloadProgress>>> {
        self.progress
            .lock()
            .map_err(|_| AppError::Unexpected("download progress lock was poisoned".into()))
    }

    pub(super) fn start(
        &self,
        path: &Path,
        root: &Path,
        input: DownloadInput,
    ) -> Result<(DownloadSummary, Option<u64>)> {
        let platform = input.platform.trim().to_owned();
        let key = download_key(&platform, &input.id)?;
        let title = validate_title(&input.title)?;
        let thumb = validate_thumb(input.thumb)?;
        let detail = input.detail;
        let chapters = normalize_input_chapters(&platform, input.chapters)?;

        let record = self.with_data(path, |data, path| {
            let mut record = match data.entries.remove(&key) {
                // An active record already owns the work. Starting it again is
                // a no-op; the caller gets the current progress instead of a
                // duplicate queue entry. A brand-new record must not take this
                // branch: it starts as `Queued` and still needs a run id.
                Some(record) if record.status.is_active() => {
                    data.entries.insert(key.clone(), record.clone());
                    return Ok(record);
                }
                Some(record) => record,
                None => DownloadRecord::new(&key, title.clone(), thumb.clone(), chapters.clone()),
            };

            record.title = title;
            record.thumb = thumb;
            record.chapters = merge_chapters(&record.chapters, chapters);
            record.detail = detail.or(record.detail);
            record.updated_at = now_millis();
            record.error = None;

            if record.chapters.iter().all(|chapter| chapter.completed) {
                record.status = DownloadStatus::Completed;
            } else {
                record.run_id = self.next_run_id();
                record.status = DownloadStatus::Queued;
                for chapter in &mut record.chapters {
                    if !chapter.completed {
                        chapter.error = None;
                    }
                }
            }

            let saved = record.clone();
            data.entries.insert(key.clone(), record);
            data.save(path)?;
            Ok(saved)
        })?;

        let spawn = (record.status == DownloadStatus::Queued).then_some(record.run_id);
        let summary = self.summary(root, &record)?;
        Ok((summary, spawn))
    }

    pub(super) fn resume(
        &self,
        path: &Path,
        root: &Path,
        platform: &str,
        id: &str,
    ) -> Result<(DownloadSummary, Option<u64>)> {
        let key = download_key(platform, id)?;
        let record = self.with_data(path, |data, path| {
            let Some(mut record) = data.entries.remove(&key) else {
                return Err(AppError::NotFound {
                    resource: "download".into(),
                });
            };

            if record.status == DownloadStatus::Completed || record.status.is_active() {
                data.entries.insert(key.clone(), record.clone());
                return Ok(record);
            }

            record.run_id = self.next_run_id();
            record.status = DownloadStatus::Queued;
            record.error = None;
            record.updated_at = now_millis();
            for chapter in &mut record.chapters {
                if !chapter.completed {
                    chapter.error = None;
                }
            }

            let saved = record.clone();
            data.entries.insert(key.clone(), record);
            data.save(path)?;
            Ok(saved)
        })?;

        let spawn = (record.status == DownloadStatus::Queued).then_some(record.run_id);
        let summary = self.summary(root, &record)?;
        Ok((summary, spawn))
    }

    pub(super) fn pause(
        &self,
        path: &Path,
        root: &Path,
        platform: &str,
        id: &str,
    ) -> Result<DownloadSummary> {
        let key = download_key(platform, id)?;
        let record = self.with_data(path, |data, path| {
            let Some(mut record) = data.entries.remove(&key) else {
                return Err(AppError::NotFound {
                    resource: "download".into(),
                });
            };

            if record.status.is_active() {
                record.run_id = self.next_run_id();
                record.status = DownloadStatus::Paused;
                record.updated_at = now_millis();
                let saved = record.clone();
                data.entries.insert(key.clone(), record);
                data.save(path)?;
                return Ok(saved);
            }

            data.entries.insert(key.clone(), record.clone());
            Ok(record)
        })?;

        self.summary(root, &record)
    }

    pub(super) fn delete(&self, path: &Path, root: &Path, platform: &str, id: &str) -> Result<()> {
        let key = download_key(platform, id)?;

        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get(&key) else {
                return Ok(());
            };
            if record.status.is_active() {
                return Err(AppError::InvalidInput {
                    field: "status",
                    reason: "pause the download before deleting it".into(),
                });
            }

            let directory = work_dir(root, record);
            if directory.exists() {
                fs::remove_dir_all(&directory)?;
            }
            data.entries.remove(&key);
            data.save(path)
        })
    }

    pub(super) fn get(
        &self,
        path: &Path,
        root: &Path,
        platform: &str,
        id: &str,
    ) -> Result<Option<DownloadDetail>> {
        let key = download_key(platform, id)?;
        let record = self.with_data(path, |data, _| Ok(data.entries.get(&key).cloned()))?;
        let Some(record) = record else {
            return Ok(None);
        };

        let summary = self.summary(root, &record)?;
        let chapters = record
            .chapters
            .iter()
            .map(|chapter| DownloadChapterSummary {
                id: chapter.id.clone(),
                title: chapter.title.clone(),
                page_count: chapter.page_count,
                completed: chapter.completed,
            })
            .collect();
        Ok(Some(DownloadDetail {
            summary,
            chapters,
            detail: record.detail.clone(),
        }))
    }

    /// Fill a missing size from the files that are already on disk. Older
    /// indexes and re-downloads over an existing directory otherwise show 0 B.
    pub(super) fn backfill_total_bytes(&self, path: &Path, root: &Path) -> Result<()> {
        self.with_data(path, |data, path| {
            if data.sizes_backfilled {
                return Ok(());
            }

            let mut changed = false;
            for record in data.entries.values_mut() {
                if record.status.is_active() || record.total_bytes != 0 {
                    continue;
                }
                let directory = work_dir(root, record);
                if !directory.exists() {
                    continue;
                }
                match work_dir_size(&directory) {
                    Ok(bytes) => {
                        record.total_bytes = bytes;
                        changed = true;
                    }
                    Err(error) => logging::warn(
                        "download",
                        format!(
                            "failed to measure the download size: platform={} id={} error={error}",
                            record.platform, record.id
                        ),
                    ),
                }
            }
            data.sizes_backfilled = true;
            if changed {
                data.save(path)?;
            }
            Ok(())
        })
    }

    pub(super) fn list(
        &self,
        path: &Path,
        root: &Path,
        offset: usize,
        limit: usize,
    ) -> Result<DownloadPage> {
        self.backfill_total_bytes(path, root)?;

        let records = self.with_data(path, |data, _| {
            let mut records: Vec<DownloadRecord> = data.entries.values().cloned().collect();
            records.sort_by(|left, right| {
                right
                    .added_at
                    .cmp(&left.added_at)
                    .then_with(|| left.platform.cmp(&right.platform))
                    .then_with(|| left.id.cmp(&right.id))
            });
            Ok(records)
        })?;

        let total = records.len();
        let entries = records
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|record| self.summary(root, &record))
            .collect::<Result<Vec<_>>>()?;

        Ok(DownloadPage { total, entries })
    }

    pub(super) fn summary(&self, root: &Path, record: &DownloadRecord) -> Result<DownloadSummary> {
        let total_chapters = record.chapters.len();
        let completed_chapters = record.chapters.iter().filter(|item| item.completed).count();
        let failed_chapters = record
            .chapters
            .iter()
            .filter(|item| !item.completed && item.error.is_some())
            .count();
        let read_chapter_id = record
            .chapters
            .iter()
            .find(|item| item.completed)
            .and_then(|item| item.id.clone());
        let cover_path = record
            .cover_file
            .as_deref()
            .filter(|name| is_safe_file_name(name))
            .map(|name| path_to_string(&work_dir(root, record).join(name)))
            .transpose()?;
        let progress = if record.status == DownloadStatus::Downloading {
            self.lock_progress()?.get(&record.key()).cloned()
        } else {
            None
        };

        Ok(DownloadSummary {
            platform: record.platform.clone(),
            id: record.id.clone(),
            title: record.title.clone(),
            thumb: record.thumb.clone(),
            cover_path,
            status: record.status,
            total_chapters,
            completed_chapters,
            failed_chapters,
            downloaded_pages: record.downloaded_pages,
            total_bytes: record.total_bytes,
            read_chapter_id,
            added_at: record.added_at,
            updated_at: record.updated_at,
            error: record.error.clone(),
            progress,
        })
    }

    pub(super) fn record_snapshot(
        &self,
        path: &Path,
        key: &DownloadKey,
    ) -> Result<Option<DownloadRecord>> {
        self.with_data(path, |data, _| Ok(data.entries.get(key).cloned()))
    }

    pub(super) fn is_current(&self, path: &Path, key: &DownloadKey, run_id: u64) -> Result<bool> {
        self.with_data(path, |data, _| {
            Ok(data.entries.get(key).is_some_and(|record| {
                record.run_id == run_id && record.status == DownloadStatus::Downloading
            }))
        })
    }

    pub(super) fn begin_worker(&self, path: &Path, key: &DownloadKey, run_id: u64) -> Result<bool> {
        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Queued {
                return Ok(false);
            }
            record.status = DownloadStatus::Downloading;
            record.updated_at = now_millis();
            data.save(path)?;
            Ok(true)
        })
    }

    /// Recount a work from disk. The worker skips pages that already exist, so
    /// their bytes must be picked up from the filesystem when a run starts.
    pub(super) fn refresh_total_bytes(
        &self,
        path: &Path,
        root: &Path,
        key: &DownloadKey,
        run_id: u64,
    ) -> Result<bool> {
        self.with_data(path, |data, path| {
            let bytes = match data.entries.get(key) {
                Some(record)
                    if record.run_id == run_id && record.status == DownloadStatus::Downloading =>
                {
                    Some(work_dir_size(&work_dir(root, record))?)
                }
                _ => None,
            };
            let Some(bytes) = bytes else {
                return Ok(false);
            };

            let changed = data
                .entries
                .get(key)
                .is_some_and(|record| record.total_bytes != bytes);
            if changed {
                if let Some(record) = data.entries.get_mut(key) {
                    record.total_bytes = bytes;
                }
                data.save(path)?;
            }
            Ok(true)
        })
    }

    pub(super) fn set_cover(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        cover_file: String,
    ) -> Result<bool> {
        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Downloading {
                return Ok(false);
            }
            record.cover_file = Some(cover_file);
            record.updated_at = now_millis();
            data.save(path)?;
            Ok(true)
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn note_chapter_started(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        chapter_index: u32,
        chapter_count: u32,
        page_count: u32,
        source_title: Option<String>,
    ) -> Result<bool> {
        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Downloading {
                return Ok(false);
            }
            let Some(chapter) = record.chapters.get_mut(chapter_index as usize) else {
                return Ok(false);
            };
            chapter.page_count = page_count;
            chapter.error = None;
            if chapter.title.is_none() {
                chapter.title = source_title;
            }
            record.updated_at = now_millis();

            self.lock_progress()?.insert(
                key.clone(),
                DownloadProgress {
                    chapter_id: chapter.id.clone(),
                    chapter_title: chapter.title.clone(),
                    // The wire field is 1-based for display; the method
                    // argument remains the 0-based chapter slot.
                    chapter_index: chapter_index.saturating_add(1),
                    chapter_count,
                    page: 0,
                    page_count,
                },
            );
            data.save(path)?;
            Ok(true)
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn note_page(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        page: u32,
        page_count: u32,
        bytes: u64,
        counted: bool,
    ) -> Result<bool> {
        self.with_data(path, |data, _| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Downloading {
                return Ok(false);
            }
            if counted {
                record.downloaded_pages = record.downloaded_pages.saturating_add(1);
                record.total_bytes = record.total_bytes.saturating_add(bytes);
            }

            if let Some(progress) = self.lock_progress()?.get_mut(key) {
                progress.page = page;
                progress.page_count = page_count;
            }
            Ok(true)
        })
    }

    pub(super) fn complete_chapter(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        chapter_index: u32,
        page_count: u32,
    ) -> Result<bool> {
        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Downloading {
                return Ok(false);
            }
            let Some(chapter) = record.chapters.get_mut(chapter_index as usize) else {
                return Ok(false);
            };
            chapter.completed = true;
            chapter.page_count = page_count;
            chapter.error = None;
            record.updated_at = now_millis();
            if let Some(progress) = self.lock_progress()?.get_mut(key) {
                progress.page = page_count;
                progress.page_count = page_count;
            }
            data.save(path)?;
            Ok(true)
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn fail_chapter(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        chapter_index: u32,
        page_count: u32,
        failure: DownloadFailure,
    ) -> Result<bool> {
        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Downloading {
                return Ok(false);
            }
            let Some(chapter) = record.chapters.get_mut(chapter_index as usize) else {
                return Ok(false);
            };
            chapter.completed = false;
            chapter.page_count = page_count;
            chapter.error = Some(failure);
            record.updated_at = now_millis();
            data.save(path)?;
            Ok(true)
        })
    }

    pub(super) fn finish_worker(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        result: Result<()>,
    ) {
        let outcome = self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(None);
            };
            if record.run_id != run_id {
                return Ok(None);
            }

            let failure = result.err().map(|error| DownloadFailure::from_app(&error));
            let completed = record.chapters.iter().filter(|item| item.completed).count();
            let all_completed = !record.chapters.is_empty() && completed == record.chapters.len();
            record.status = if all_completed {
                DownloadStatus::Completed
            } else if completed > 0 {
                DownloadStatus::Partial
            } else {
                DownloadStatus::Failed
            };
            record.downloaded_pages = record
                .chapters
                .iter()
                .filter(|item| item.completed)
                .map(|item| item.page_count)
                .sum();
            record.error = if record.status == DownloadStatus::Completed {
                None
            } else {
                failure.or_else(|| {
                    record
                        .chapters
                        .iter()
                        .rev()
                        .find_map(|item| item.error.clone())
                })
            };
            record.updated_at = now_millis();
            let status = record.status;
            let error = record.error.clone();
            data.save(path)?;
            Ok(Some((status, error)))
        });

        match outcome {
            Ok(Some((status, error))) => match status {
                DownloadStatus::Completed => logging::info(
                    "download",
                    format!("download completed: platform={} id={}", key.0, key.1),
                ),
                _ => logging::warn(
                    "download",
                    format!(
                        "download finished with status {:?}: platform={} id={} error={:?}",
                        status, key.0, key.1, error
                    ),
                ),
            },
            Ok(None) => {}
            Err(error) => logging::error(
                "download",
                format!(
                    "failed to finalize download: platform={} id={} error={error}",
                    key.0, key.1
                ),
            ),
        }
    }

    pub(super) fn read_local_chapter(
        &self,
        path: &Path,
        root: &Path,
        platform: &str,
        id: &str,
        chapter_id: Option<&str>,
    ) -> Result<Option<ReaderChapter>> {
        local::read_local_chapter(self, path, root, platform, id, chapter_id)
    }
}

impl DownloadsData {
    pub(super) fn load(&mut self, path: &Path) -> Result<()> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };

        let document = match serde_json::from_slice::<DownloadDocument>(&bytes) {
            Ok(document) => document,
            Err(error) => {
                logging::warn(
                    "download",
                    format!("failed to parse downloads; treating it as empty: {error}"),
                );
                self.corrupt = true;
                return Ok(());
            }
        };
        if document.version != DOCUMENT_VERSION {
            return Err(AppError::Unexpected(format!(
                "unsupported downloads document version {}",
                document.version
            )));
        }

        self.entries.clear();
        let mut needs_save = false;
        for record in document.entries {
            let Some(mut record) = record.sanitized() else {
                needs_save = true;
                logging::warn("download", "skipping an invalid download entry");
                continue;
            };
            if record.status.is_active() {
                record.status = DownloadStatus::Paused;
                needs_save = true;
            }
            if self.entries.insert(record.key(), record).is_some() {
                needs_save = true;
            }
        }

        if needs_save && let Err(error) = self.save(path) {
            logging::warn(
                "download",
                format!("failed to normalize downloads: {error}"),
            );
        }
        Ok(())
    }

    pub(super) fn save(&mut self, path: &Path) -> Result<()> {
        if self.corrupt {
            self.backup_corrupt(path);
        }

        let mut entries: Vec<DownloadRecord> = self.entries.values().cloned().collect();
        entries.sort_by(|left, right| {
            left.added_at
                .cmp(&right.added_at)
                .then_with(|| left.platform.cmp(&right.platform))
                .then_with(|| left.id.cmp(&right.id))
        });

        write_document(
            path,
            &DownloadDocument {
                version: DOCUMENT_VERSION,
                entries,
            },
        )?;
        self.corrupt = false;
        Ok(())
    }

    pub(super) fn backup_corrupt(&mut self, path: &Path) {
        match move_corrupt_file(path) {
            Ok(Some(backup)) => logging::warn(
                "download",
                format!(
                    "backed up an unreadable downloads file to {}",
                    backup.display()
                ),
            ),
            Ok(None) => {}
            Err(error) => logging::warn(
                "download",
                format!("failed to back up the unreadable downloads file: {error}"),
            ),
        }
    }
}
