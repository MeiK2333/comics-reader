//! Favorites persisted as a small versioned JSON document.
//!
//! Favorites are user-curated and change only when the user toggles a work, so
//! the whole list is rewritten atomically instead of using history's
//! append-only event log. That keeps add, move and remove symmetric and
//! replay-free.
//!
//! The file lives in the app data directory and is loaded into memory on the
//! first command. A malformed document is logged and treated as empty; the next
//! write backs the unreadable file up to `favorites.json.bak` before replacing
//! it, so a broken file cannot be overwritten silently.
//!
//! Version 2 added groups. Version 1 entries have no `groupId`; they migrate to
//! the built-in default group on load.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, Result};
use crate::logging;
use crate::platforms::is_known_platform;
use crate::storage::{
    atomic_write_json, backup_corrupt as move_corrupt_file, now_millis, page_window,
};

const FAVORITES_FILE_NAME: &str = "favorites.json";
const DOCUMENT_VERSION: u32 = 2;
const DEFAULT_GROUP_ID: &str = "default";
const MAX_ID_CHARS: usize = 256;
const MAX_TITLE_CHARS: usize = 2_000;
const MAX_THUMB_CHARS: usize = 4_096;
const MAX_GROUP_ID_CHARS: usize = 64;
const MAX_GROUP_NAME_CHARS: usize = 40;

type FavoriteKey = (String, String);

fn default_group_id() -> String {
    DEFAULT_GROUP_ID.to_owned()
}

/// One favorite row. `added_at` is assigned by Rust so a client clock cannot
/// reorder the list, and it is kept when the same work is added again.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FavoriteEntry {
    platform: String,
    id: String,
    title: String,
    thumb: Option<String>,
    added_at: u64,
    #[serde(default = "default_group_id")]
    group_id: String,
}

impl FavoriteEntry {
    fn key(&self) -> FavoriteKey {
        (self.platform.clone(), self.id.clone())
    }

    /// Drop junk that a hand-edited or partially written file could contain.
    fn sanitized(mut self) -> Option<Self> {
        if !is_known_platform(&self.platform) {
            return None;
        }

        self.id = self.id.trim().to_owned();
        self.title = self.title.trim().to_owned();
        if self.id.is_empty() || self.title.is_empty() || self.added_at == 0 {
            return None;
        }
        if self.id.chars().count() > MAX_ID_CHARS || self.title.chars().count() > MAX_TITLE_CHARS {
            return None;
        }

        // A broken thumbnail still leaves a usable favorite: the list falls
        // back to the title placeholder instead of dropping the whole row.
        self.thumb = self
            .thumb
            .take()
            .and_then(non_empty)
            .filter(|value| value.chars().count() <= MAX_THUMB_CHARS);

        self.group_id = self.group_id.trim().to_owned();
        if self.group_id.is_empty() || self.group_id.chars().count() > MAX_GROUP_ID_CHARS {
            self.group_id = default_group_id();
        }
        Some(self)
    }
}

/// A grouping folder. The default group is synthesized with an empty name so
/// the UI can localize it; custom groups keep their user-entered name.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FavoriteGroup {
    id: String,
    name: String,
    created_at: u64,
}

impl FavoriteGroup {
    fn default_group() -> Self {
        Self {
            id: default_group_id(),
            name: String::new(),
            created_at: 0,
        }
    }

    fn sanitized(mut self) -> Option<Self> {
        self.id = self.id.trim().to_owned();
        self.name = self.name.trim().to_owned();
        if self.id.is_empty()
            || self.id == DEFAULT_GROUP_ID
            || self.id.chars().count() > MAX_GROUP_ID_CHARS
            || self.name.is_empty()
            || self.name.chars().count() > MAX_GROUP_NAME_CHARS
        {
            return None;
        }
        Some(self)
    }
}

/// One group plus its current number of favorites.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FavoriteGroupSummary {
    id: String,
    name: String,
    created_at: u64,
    count: usize,
}

/// All groups plus the total number of favorites across groups.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FavoriteGroupList {
    total: usize,
    groups: Vec<FavoriteGroupSummary>,
}

/// Payload sent by the detail page when the user favorites or moves a work.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FavoriteInput {
    platform: String,
    id: String,
    title: String,
    thumb: Option<String>,
    group_id: Option<String>,
}

impl FavoriteInput {
    /// Return the entry plus the explicitly requested group. Keeping the two
    /// separate lets an idempotent add preserve an existing custom group when
    /// the caller did not choose one.
    fn into_parts(self, added_at: u64) -> Result<(FavoriteEntry, Option<String>)> {
        let (platform, id) = favorite_key(&self.platform, &self.id)?;

        let title = self.title.trim();
        if title.is_empty() {
            return Err(invalid("title", "must not be empty"));
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(invalid("title", "is too long"));
        }

        let thumb = self.thumb.and_then(non_empty);
        if let Some(value) = thumb.as_ref()
            && value.chars().count() > MAX_THUMB_CHARS
        {
            return Err(invalid("thumb", "is too long"));
        }

        let requested_group = match self.group_id {
            Some(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    if trimmed.chars().count() > MAX_GROUP_ID_CHARS {
                        return Err(invalid("groupId", "is too long"));
                    }
                    Some(trimmed.to_owned())
                }
            }
            None => None,
        };

        Ok((
            FavoriteEntry {
                platform,
                id,
                title: title.to_owned(),
                thumb,
                added_at,
                group_id: requested_group.clone().unwrap_or_else(default_group_id),
            },
            requested_group,
        ))
    }
}

/// One page of favorites plus the total number of works in that filter, so the
/// UI can paginate without loading every entry over IPC.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FavoritePage {
    total: usize,
    entries: Vec<FavoriteEntry>,
}

/// The on-disk shape. Keeping a version lets a future release migrate the file
/// explicitly instead of guessing from the entries.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct FavoritesDocument {
    version: u32,
    #[serde(default)]
    groups: Vec<FavoriteGroup>,
    entries: Vec<FavoriteEntry>,
}

/// Tauri-managed store. The file is loaded on the first command and then
/// indexed in memory; every operation runs under the same lock.
#[derive(Default)]
pub(crate) struct FavoritesStore {
    data: Mutex<FavoritesData>,
}

struct FavoritesData {
    loaded: bool,
    groups: HashMap<String, FavoriteGroup>,
    entries: HashMap<FavoriteKey, FavoriteEntry>,
    /// The file could not be parsed, so the next write must preserve it.
    corrupt: bool,
}

impl Default for FavoritesData {
    fn default() -> Self {
        let mut groups = HashMap::new();
        groups.insert(DEFAULT_GROUP_ID.to_owned(), FavoriteGroup::default_group());
        Self {
            loaded: false,
            groups,
            entries: HashMap::new(),
            corrupt: false,
        }
    }
}

impl FavoritesStore {
    fn with_data<T>(
        &self,
        path: &Path,
        operation: impl FnOnce(&mut FavoritesData, &Path) -> Result<T>,
    ) -> Result<T> {
        let mut data = self
            .data
            .lock()
            .map_err(|_| AppError::Unexpected("favorites lock was poisoned".into()))?;
        if !data.loaded {
            data.load(path)?;
            data.loaded = true;
        }
        operation(&mut data, path)
    }
}

impl FavoritesData {
    fn load(&mut self, path: &Path) -> Result<()> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };

        let document = match serde_json::from_slice::<FavoritesDocument>(&bytes) {
            Ok(document) => document,
            Err(error) => {
                logging::warn(
                    "favorites",
                    format!("failed to parse favorites; treating it as empty: {error}"),
                );
                self.corrupt = true;
                return Ok(());
            }
        };

        if document.version != 1 && document.version != DOCUMENT_VERSION {
            return Err(AppError::Unexpected(format!(
                "unsupported favorites document version {}",
                document.version
            )));
        }

        let has_default_group = document
            .groups
            .iter()
            .any(|group| group.id.trim() == DEFAULT_GROUP_ID);
        self.groups.clear();
        self.groups
            .insert(DEFAULT_GROUP_ID.to_owned(), FavoriteGroup::default_group());
        self.entries.clear();

        let mut needs_save = document.version == 1 || !has_default_group;

        if document.version == DOCUMENT_VERSION {
            for group in document.groups {
                if group.id.trim() == DEFAULT_GROUP_ID {
                    if !group.name.trim().is_empty() {
                        needs_save = true;
                    }
                    continue;
                }

                let Some(group) = group.sanitized() else {
                    needs_save = true;
                    logging::warn("favorites", "skipping an invalid favorite group");
                    continue;
                };
                if self.groups.contains_key(&group.id) {
                    needs_save = true;
                    logging::warn("favorites", "skipping a duplicate favorite group");
                    continue;
                }
                if group_name_taken(&self.groups, &group.name, None) {
                    needs_save = true;
                    logging::warn("favorites", "skipping a duplicate group name");
                    continue;
                }
                self.groups.insert(group.id.clone(), group);
            }
        }

        for entry in document.entries {
            let Some(mut entry) = entry.sanitized() else {
                needs_save = true;
                logging::warn("favorites", "skipping an invalid favorite entry");
                continue;
            };
            if !self.groups.contains_key(&entry.group_id) {
                entry.group_id = default_group_id();
                needs_save = true;
            }
            let key = entry.key();
            if self.entries.insert(key, entry).is_some() {
                needs_save = true;
            }
        }

        if needs_save && let Err(error) = self.save(path) {
            logging::warn("favorites", format!("failed to migrate favorites: {error}"));
        }
        Ok(())
    }

    fn add(&mut self, path: &Path, input: FavoriteInput) -> Result<FavoriteEntry> {
        let (mut entry, requested_group) = input.into_parts(now_millis())?;

        if let Some(group_id) = requested_group.as_deref()
            && !self.groups.contains_key(group_id)
        {
            return Err(invalid(
                "groupId",
                "must reference an existing favorite group",
            ));
        }

        // Adding an existing work refreshes its snapshot without moving it to
        // the top or changing its group unless one was explicitly selected.
        if let Some(existing) = self.entries.get(&entry.key()) {
            entry.added_at = existing.added_at;
            if requested_group.is_none() {
                entry.group_id = existing.group_id.clone();
            }
        }

        let saved = entry.clone();
        let mut next_entries = self.entries.clone();
        next_entries.insert(entry.key(), entry);
        let groups = self.groups.clone();
        self.commit(path, groups, next_entries)?;
        Ok(saved)
    }

    fn remove(&mut self, path: &Path, platform: &str, id: &str) -> Result<()> {
        let key = favorite_key(platform, id)?;
        if !self.entries.contains_key(&key) {
            return Ok(());
        }

        let mut next_entries = self.entries.clone();
        next_entries.remove(&key);
        let groups = self.groups.clone();
        self.commit(path, groups, next_entries)
    }

    fn get(&self, platform: &str, id: &str) -> Result<Option<FavoriteEntry>> {
        let key = favorite_key(platform, id)?;
        Ok(self.entries.get(&key).cloned())
    }

    fn list(&self, offset: usize, limit: usize, group_id: Option<&str>) -> FavoritePage {
        let group_id = group_id.map(str::trim).filter(|value| !value.is_empty());
        let mut entries: Vec<FavoriteEntry> = self
            .entries
            .values()
            .filter(|entry| group_id.is_none_or(|group| entry.group_id == group))
            .cloned()
            .collect();
        entries.sort_by(|left, right| {
            right
                .added_at
                .cmp(&left.added_at)
                .then_with(|| left.platform.cmp(&right.platform))
                .then_with(|| left.id.cmp(&right.id))
        });

        let total = entries.len();
        let entries = entries.into_iter().skip(offset).take(limit).collect();
        FavoritePage { total, entries }
    }

    fn list_groups(&self) -> FavoriteGroupList {
        let mut groups: Vec<FavoriteGroup> = self.groups.values().cloned().collect();
        if !groups.iter().any(|group| group.id == DEFAULT_GROUP_ID) {
            groups.push(FavoriteGroup::default_group());
        }
        groups.sort_by(group_order);

        let mut counts: HashMap<&str, usize> = HashMap::new();
        for entry in self.entries.values() {
            *counts.entry(entry.group_id.as_str()).or_default() += 1;
        }

        FavoriteGroupList {
            total: self.entries.len(),
            groups: groups
                .into_iter()
                .map(|group| FavoriteGroupSummary {
                    count: counts.get(group.id.as_str()).copied().unwrap_or(0),
                    id: group.id,
                    name: group.name,
                    created_at: group.created_at,
                })
                .collect(),
        }
    }

    fn create_group(&mut self, path: &Path, name: &str) -> Result<FavoriteGroupSummary> {
        let name = validate_group_name(name)?;
        if group_name_taken(&self.groups, &name, None) {
            return Err(invalid("name", "is already used by another group"));
        }

        let id = next_group_id(&self.groups);
        let created_at = now_millis();
        let group = FavoriteGroup {
            id: id.clone(),
            name: name.clone(),
            created_at,
        };

        let mut next_groups = self.groups.clone();
        next_groups.insert(id.clone(), group);
        let entries = self.entries.clone();
        self.commit(path, next_groups, entries)?;

        Ok(FavoriteGroupSummary {
            id,
            name,
            created_at,
            count: 0,
        })
    }

    fn rename_group(&mut self, path: &Path, group_id: &str, name: &str) -> Result<()> {
        let group_id = group_id.trim();
        if group_id.is_empty() {
            return Err(invalid("groupId", "must not be empty"));
        }
        if group_id == DEFAULT_GROUP_ID {
            return Err(invalid("groupId", "the default group cannot be renamed"));
        }
        if !self.groups.contains_key(group_id) {
            return Err(invalid("groupId", "does not exist"));
        }

        let name = validate_group_name(name)?;
        if group_name_taken(&self.groups, &name, Some(group_id)) {
            return Err(invalid("name", "is already used by another group"));
        }

        let mut next_groups = self.groups.clone();
        if let Some(group) = next_groups.get_mut(group_id) {
            group.name = name;
        }
        let entries = self.entries.clone();
        self.commit(path, next_groups, entries)
    }

    fn delete_group(&mut self, path: &Path, group_id: &str) -> Result<()> {
        let group_id = group_id.trim();
        if group_id.is_empty() {
            return Err(invalid("groupId", "must not be empty"));
        }
        if group_id == DEFAULT_GROUP_ID {
            return Err(invalid("groupId", "the default group cannot be deleted"));
        }
        if !self.groups.contains_key(group_id) {
            return Ok(());
        }

        let mut next_groups = self.groups.clone();
        next_groups.remove(group_id);
        let mut next_entries = self.entries.clone();
        for entry in next_entries.values_mut() {
            if entry.group_id == group_id {
                entry.group_id = default_group_id();
            }
        }
        self.commit(path, next_groups, next_entries)
    }

    fn save(&mut self, path: &Path) -> Result<()> {
        self.groups
            .entry(DEFAULT_GROUP_ID.to_owned())
            .or_insert_with(FavoriteGroup::default_group);
        if self.corrupt {
            self.backup_corrupt(path);
        }
        write_document(path, &self.groups, &self.entries)?;
        self.corrupt = false;
        Ok(())
    }

    /// Write candidate maps before swapping them into memory. A failed write
    /// therefore leaves the visible state consistent with the file on disk.
    fn commit(
        &mut self,
        path: &Path,
        mut groups: HashMap<String, FavoriteGroup>,
        entries: HashMap<FavoriteKey, FavoriteEntry>,
    ) -> Result<()> {
        groups
            .entry(DEFAULT_GROUP_ID.to_owned())
            .or_insert_with(FavoriteGroup::default_group);
        if self.corrupt {
            self.backup_corrupt(path);
        }
        write_document(path, &groups, &entries)?;
        self.groups = groups;
        self.entries = entries;
        self.corrupt = false;
        Ok(())
    }

    fn backup_corrupt(&mut self, path: &Path) {
        match move_corrupt_file(path) {
            Ok(Some(backup)) => logging::warn(
                "favorites",
                format!(
                    "backed up an unreadable favorites file to {}",
                    backup.display()
                ),
            ),
            Ok(None) => {}
            Err(error) => logging::warn(
                "favorites",
                format!("failed to back up the unreadable favorites file: {error}"),
            ),
        }
    }
}

fn write_document(
    path: &Path,
    groups: &HashMap<String, FavoriteGroup>,
    entries: &HashMap<FavoriteKey, FavoriteEntry>,
) -> Result<()> {
    let mut groups: Vec<FavoriteGroup> = groups.values().cloned().collect();
    if !groups.iter().any(|group| group.id == DEFAULT_GROUP_ID) {
        groups.push(FavoriteGroup::default_group());
    }
    groups.sort_by(group_order);

    let mut entries: Vec<FavoriteEntry> = entries.values().cloned().collect();
    entries.sort_by(|left, right| {
        left.added_at
            .cmp(&right.added_at)
            .then_with(|| left.platform.cmp(&right.platform))
            .then_with(|| left.id.cmp(&right.id))
    });

    atomic_write_json(
        path,
        &FavoritesDocument {
            version: DOCUMENT_VERSION,
            groups,
            entries,
        },
    )
}

fn group_order(left: &FavoriteGroup, right: &FavoriteGroup) -> std::cmp::Ordering {
    (left.id != DEFAULT_GROUP_ID)
        .cmp(&(right.id != DEFAULT_GROUP_ID))
        .then_with(|| left.created_at.cmp(&right.created_at))
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left.id.cmp(&right.id))
}

fn group_name_taken(
    groups: &HashMap<String, FavoriteGroup>,
    name: &str,
    except: Option<&str>,
) -> bool {
    let name = name.to_lowercase();
    groups.values().any(|group| {
        group.id != DEFAULT_GROUP_ID
            && Some(group.id.as_str()) != except
            && group.name.to_lowercase() == name
    })
}

fn next_group_id(groups: &HashMap<String, FavoriteGroup>) -> String {
    let base = format!("g{}", now_millis());
    if !groups.contains_key(&base) {
        return base;
    }

    let mut suffix = 1;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !groups.contains_key(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

/// Validate the pair used as the stable identity of a work.
fn favorite_key(platform: &str, id: &str) -> Result<FavoriteKey> {
    let platform = platform.trim();
    if !is_known_platform(platform) {
        return Err(invalid("platform", "must be a supported platform"));
    }

    let id = id.trim();
    if id.is_empty() {
        return Err(invalid("id", "must not be empty"));
    }
    if id.chars().count() > MAX_ID_CHARS {
        return Err(invalid("id", "is too long"));
    }

    Ok((platform.to_owned(), id.to_owned()))
}

fn validate_group_name(value: &str) -> Result<String> {
    let name = value.trim();
    if name.is_empty() {
        return Err(invalid("name", "must not be empty"));
    }
    if name.chars().count() > MAX_GROUP_NAME_CHARS {
        return Err(invalid("name", "is too long"));
    }
    Ok(name.to_owned())
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn invalid(field: &'static str, reason: &str) -> AppError {
    AppError::InvalidInput {
        field,
        reason: reason.to_owned(),
    }
}

fn favorites_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(FAVORITES_FILE_NAME))
}

/// Return one page of favorites, optionally filtered by group, newest first.
#[tauri::command]
pub(crate) async fn list_favorites(
    app: AppHandle,
    offset: Option<u32>,
    limit: Option<u32>,
    group_id: Option<String>,
) -> Result<FavoritePage> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = favorites_path(&app)?;
        let store = app.state::<FavoritesStore>();
        let (offset, limit) = page_window(offset, limit);
        let group_id = group_id.as_deref();
        store.with_data(&path, |data, _| Ok(data.list(offset, limit, group_id)))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("favorites task failed: {error}")))?
}

/// Look up one saved work, including its current group.
#[tauri::command]
pub(crate) async fn get_favorite(
    app: AppHandle,
    platform: String,
    id: String,
) -> Result<Option<FavoriteEntry>> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = favorites_path(&app)?;
        let store = app.state::<FavoritesStore>();
        store.with_data(&path, |data, _| data.get(&platform, &id))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("favorites task failed: {error}")))?
}

/// Insert, refresh or move one favorite. Returns the saved row.
#[tauri::command]
pub(crate) async fn add_favorite(app: AppHandle, entry: FavoriteInput) -> Result<FavoriteEntry> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = favorites_path(&app)?;
        let store = app.state::<FavoritesStore>();
        store.with_data(&path, |data, path| data.add(path, entry))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("favorites task failed: {error}")))?
}

/// Remove one favorite. Removing a work that is not saved is a no-op.
#[tauri::command]
pub(crate) async fn remove_favorite(app: AppHandle, platform: String, id: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = favorites_path(&app)?;
        let store = app.state::<FavoritesStore>();
        store.with_data(&path, |data, path| data.remove(path, &platform, &id))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("favorites task failed: {error}")))?
}

/// Return every group and the number of favorites in each.
#[tauri::command]
pub(crate) async fn list_favorite_groups(app: AppHandle) -> Result<FavoriteGroupList> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = favorites_path(&app)?;
        let store = app.state::<FavoritesStore>();
        store.with_data(&path, |data, _| Ok(data.list_groups()))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("favorites task failed: {error}")))?
}

/// Create a custom group.
#[tauri::command]
pub(crate) async fn create_favorite_group(
    app: AppHandle,
    name: String,
) -> Result<FavoriteGroupSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = favorites_path(&app)?;
        let store = app.state::<FavoritesStore>();
        store.with_data(&path, |data, path| data.create_group(path, &name))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("favorites task failed: {error}")))?
}

/// Rename a custom group.
#[tauri::command]
pub(crate) async fn rename_favorite_group(
    app: AppHandle,
    group_id: String,
    name: String,
) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = favorites_path(&app)?;
        let store = app.state::<FavoritesStore>();
        store.with_data(&path, |data, path| {
            data.rename_group(path, &group_id, &name)
        })
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("favorites task failed: {error}")))?
}

/// Delete a custom group, moving its favorites to the default group.
#[tauri::command]
pub(crate) async fn delete_favorite_group(app: AppHandle, group_id: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = favorites_path(&app)?;
        let store = app.state::<FavoritesStore>();
        store.with_data(&path, |data, path| data.delete_group(path, &group_id))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("favorites task failed: {error}")))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::storage::{backup_path, temporary_path};

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "comics-reader-favorites-{label}-{}-{}.json",
            std::process::id(),
            now_millis()
        ))
    }

    fn entry(id: &str, added_at: u64) -> FavoriteEntry {
        FavoriteEntry {
            platform: "nhentai".into(),
            id: id.into(),
            title: format!("Gallery {id}"),
            thumb: Some("https://t.nhentai.net/galleries/1/thumb.webp".into()),
            added_at,
            group_id: default_group_id(),
        }
    }

    fn input(id: &str) -> FavoriteInput {
        FavoriteInput {
            platform: "nhentai".into(),
            id: id.into(),
            title: format!("Gallery {id}"),
            thumb: Some("https://t.nhentai.net/galleries/1/thumb.webp".into()),
            group_id: None,
        }
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(temporary_path(path));
        let _ = fs::remove_file(backup_path(path));
    }

    #[test]
    fn serializes_documents_with_groups_and_camel_case() {
        let value = serde_json::to_value(FavoritesDocument {
            version: DOCUMENT_VERSION,
            groups: vec![
                FavoriteGroup::default_group(),
                FavoriteGroup {
                    id: "g1".into(),
                    name: "Later".into(),
                    created_at: 100,
                },
            ],
            entries: vec![FavoriteEntry {
                group_id: "g1".into(),
                ..entry("42", 1_700_000_000_000)
            }],
        })
        .unwrap();

        assert_eq!(value["version"], DOCUMENT_VERSION);
        assert_eq!(value["groups"][0]["id"], DEFAULT_GROUP_ID);
        assert_eq!(value["groups"][1]["name"], "Later");
        assert_eq!(value["entries"][0]["groupId"], "g1");
        assert_eq!(value["entries"][0]["addedAt"], 1_700_000_000_000_u64);
        assert!(value["entries"][0].get("group_id").is_none());
    }

    #[test]
    fn migrates_version_one_entries_to_the_default_group() {
        let path = temp_path("migrate");
        let version_one = serde_json::json!({
            "version": 1,
            "entries": [{
                "platform": "nhentai",
                "id": "42",
                "title": "Gallery 42",
                "thumb": null,
                "addedAt": 1_700_000_000_000_u64,
            }]
        });
        fs::write(&path, serde_json::to_vec(&version_one).unwrap()).unwrap();

        let mut data = FavoritesData::default();
        data.load(&path).unwrap();

        assert_eq!(data.entries.len(), 1);
        let stored = data
            .entries
            .get(&("nhentai".into(), "42".into()))
            .expect("entry should migrate");
        assert_eq!(stored.group_id, DEFAULT_GROUP_ID);
        assert!(data.groups.contains_key(DEFAULT_GROUP_ID));

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["version"], DOCUMENT_VERSION);
        assert_eq!(saved["entries"][0]["groupId"], DEFAULT_GROUP_ID);
        assert_eq!(saved["groups"][0]["id"], DEFAULT_GROUP_ID);

        cleanup(&path);
    }

    #[test]
    fn adds_lists_and_filters_by_group() {
        let path = temp_path("groups");
        let mut data = FavoritesData::default();
        data.load(&path).unwrap();

        let first = data.add(&path, input("1")).unwrap();
        assert_eq!(first.group_id, DEFAULT_GROUP_ID);

        let group = data.create_group(&path, "Later").unwrap();
        let mut custom = input("2");
        custom.group_id = Some(group.id.clone());
        let second = data.add(&path, custom).unwrap();
        assert_eq!(second.group_id, group.id);

        assert_eq!(data.list(0, 50, None).total, 2);
        assert_eq!(data.list(0, 50, Some(DEFAULT_GROUP_ID)).total, 1);
        assert_eq!(data.list(0, 50, Some(group.id.as_str())).total, 1);
        assert!(data.get("nhentai", "1").unwrap().is_some());
        assert!(data.get("nhentai", "missing").unwrap().is_none());

        cleanup(&path);
    }

    #[test]
    fn adding_without_a_group_preserves_an_existing_custom_group() {
        let path = temp_path("preserve");
        let mut data = FavoritesData::default();
        data.load(&path).unwrap();

        let group = data.create_group(&path, "Later").unwrap();
        let mut custom = input("1");
        custom.group_id = Some(group.id.clone());
        let saved = data.add(&path, custom).unwrap();

        let refreshed = data.add(&path, input("1")).unwrap();

        assert_eq!(refreshed.group_id, group.id);
        assert_eq!(refreshed.added_at, saved.added_at);

        cleanup(&path);
    }

    #[test]
    fn lists_group_counts_with_default_first() {
        let path = temp_path("counts");
        let mut data = FavoritesData::default();
        data.load(&path).unwrap();

        data.add(&path, input("1")).unwrap();
        data.add(&path, input("2")).unwrap();
        let group = data.create_group(&path, "Later").unwrap();
        let mut custom = input("3");
        custom.group_id = Some(group.id.clone());
        data.add(&path, custom).unwrap();

        let list = data.list_groups();

        assert_eq!(list.total, 3);
        assert_eq!(list.groups[0].id, DEFAULT_GROUP_ID);
        assert_eq!(list.groups[0].count, 2);
        assert_eq!(list.groups[1].id, group.id);
        assert_eq!(list.groups[1].count, 1);

        cleanup(&path);
    }

    #[test]
    fn rejects_invalid_and_duplicate_group_names() {
        let path = temp_path("group-names");
        let mut data = FavoritesData::default();
        data.load(&path).unwrap();

        data.create_group(&path, "Later").unwrap();
        assert_eq!(
            data.create_group(&path, "later").unwrap_err().code(),
            ErrorCode::InvalidInput
        );
        assert_eq!(
            data.create_group(&path, "  ").unwrap_err().code(),
            ErrorCode::InvalidInput
        );
        assert_eq!(
            data.create_group(&path, &"x".repeat(MAX_GROUP_NAME_CHARS + 1))
                .unwrap_err()
                .code(),
            ErrorCode::InvalidInput
        );

        assert_eq!(
            data.rename_group(&path, DEFAULT_GROUP_ID, "Nope")
                .unwrap_err()
                .code(),
            ErrorCode::InvalidInput
        );
        assert_eq!(
            data.delete_group(&path, DEFAULT_GROUP_ID)
                .unwrap_err()
                .code(),
            ErrorCode::InvalidInput
        );

        cleanup(&path);
    }

    #[test]
    fn deleting_a_group_moves_its_favorites_to_default() {
        let path = temp_path("delete-group");
        let mut data = FavoritesData::default();
        data.load(&path).unwrap();

        let group = data.create_group(&path, "Later").unwrap();
        let mut custom = input("1");
        custom.group_id = Some(group.id.clone());
        data.add(&path, custom).unwrap();

        data.delete_group(&path, &group.id).unwrap();

        let stored = data
            .get("nhentai", "1")
            .unwrap()
            .expect("entry should remain");
        assert_eq!(stored.group_id, DEFAULT_GROUP_ID);
        assert!(!data.groups.contains_key(&group.id));

        cleanup(&path);
    }

    #[test]
    fn loads_version_two_and_maps_unknown_groups_to_default() {
        let path = temp_path("v2-unknown-group");
        let document = serde_json::json!({
            "version": 2,
            "groups": [{ "id": "default", "name": "", "createdAt": 0 }],
            "entries": [{
                "platform": "nhentai",
                "id": "1",
                "title": "Gallery 1",
                "thumb": null,
                "addedAt": 1_700_000_000_000_u64,
                "groupId": "missing",
            }]
        });
        fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();

        let mut data = FavoritesData::default();
        data.load(&path).unwrap();

        assert_eq!(
            data.get("nhentai", "1").unwrap().unwrap().group_id,
            DEFAULT_GROUP_ID
        );

        cleanup(&path);
    }

    #[test]
    fn malformed_file_is_treated_as_empty_and_backed_up() {
        let path = temp_path("malformed");
        fs::write(&path, "not json").unwrap();

        let mut data = FavoritesData::default();
        data.load(&path).unwrap();
        assert!(data.entries.is_empty());
        assert!(data.corrupt);

        data.add(&path, input("1")).unwrap();
        assert!(!data.corrupt);
        assert!(backup_path(&path).exists());

        let mut reloaded = FavoritesData::default();
        reloaded.load(&path).unwrap();
        assert_eq!(reloaded.entries.len(), 1);

        cleanup(&path);
    }

    #[test]
    fn rejects_unsupported_document_versions() {
        let path = temp_path("version");
        let document = serde_json::json!({
            "version": 3,
            "groups": [],
            "entries": []
        });
        fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();

        let mut data = FavoritesData::default();
        let error = data.load(&path).unwrap_err();

        assert_eq!(error.code(), ErrorCode::Unexpected);

        cleanup(&path);
    }

    #[test]
    fn renames_a_custom_group() {
        let path = temp_path("rename-group");
        let mut data = FavoritesData::default();
        data.load(&path).unwrap();

        let group = data.create_group(&path, "Later").unwrap();
        data.rename_group(&path, &group.id, "Renamed").unwrap();

        let stored = data.groups.get(&group.id).expect("group should exist");
        assert_eq!(stored.name, "Renamed");

        let list = data.list_groups();
        assert_eq!(list.groups[1].id, group.id);
        assert_eq!(list.groups[1].name, "Renamed");

        cleanup(&path);
    }

    #[test]
    fn rejects_an_unknown_group_when_adding() {
        let path = temp_path("unknown-group");
        let mut data = FavoritesData::default();
        data.load(&path).unwrap();

        let mut custom = input("1");
        custom.group_id = Some("missing".into());
        assert_eq!(
            data.add(&path, custom).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        cleanup(&path);
    }

    #[test]
    fn remove_is_idempotent_and_does_not_write() {
        let path = temp_path("remove-missing");
        let mut data = FavoritesData::default();
        data.load(&path).unwrap();

        data.remove(&path, "nhentai", "missing").unwrap();

        assert!(!path.exists());

        cleanup(&path);
    }

    #[test]
    fn rejects_invalid_input() {
        let mut value = input("1");
        value.platform = "unknown".into();
        assert_eq!(
            value.into_parts(0).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        let mut value = input("1");
        value.id = "  ".into();
        assert_eq!(
            value.into_parts(0).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        let mut value = input("1");
        value.title = "  ".into();
        assert_eq!(
            value.into_parts(0).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        let mut value = input("1");
        value.title = "x".repeat(MAX_TITLE_CHARS + 1);
        assert_eq!(
            value.into_parts(0).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        let mut value = input("1");
        value.thumb = Some("x".repeat(MAX_THUMB_CHARS + 1));
        assert_eq!(
            value.into_parts(0).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        let mut value = input("1");
        value.group_id = Some("x".repeat(MAX_GROUP_ID_CHARS + 1));
        assert_eq!(
            value.into_parts(0).unwrap_err().code(),
            ErrorCode::InvalidInput
        );
    }
}
