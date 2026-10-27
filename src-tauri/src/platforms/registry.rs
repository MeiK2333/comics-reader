//! The single source of platform ids, profiles and catalog capabilities.

use url::Url;

use crate::error::{AppError, Result};

use super::copymanga::Copymanga;
use super::generic::Generic;
use super::jm::Jm;
use super::manhuagui::Manhuagui;
use super::nhentai::Nhentai;
use super::{Catalog, ImageProfile};

struct Registration {
    profile: &'static dyn ImageProfile,
    catalog: Option<&'static dyn Catalog>,
}

impl Registration {
    const fn image_only(profile: &'static dyn ImageProfile) -> Self {
        Self {
            profile,
            catalog: None,
        }
    }

    const fn catalog(profile: &'static dyn ImageProfile, catalog: &'static dyn Catalog) -> Self {
        Self {
            profile,
            catalog: Some(catalog),
        }
    }
}

static GENERIC: Generic = Generic;
static MANHUAGUI: Manhuagui = Manhuagui;
static JM: Jm = Jm::new();
static NHENTAI: Nhentai = Nhentai;
static COPYMANGA: Copymanga = Copymanga;

static REGISTRY: [Registration; 5] = [
    Registration::image_only(&GENERIC),
    Registration::catalog(&MANHUAGUI, &MANHUAGUI),
    Registration::catalog(&JM, &JM),
    Registration::catalog(&NHENTAI, &NHENTAI),
    Registration::catalog(&COPYMANGA, &COPYMANGA),
];

/// Resolve an explicit image profile id, or detect one from the image URL.
pub(crate) fn resolve(explicit: Option<&str>, url: &Url) -> Result<&'static dyn ImageProfile> {
    match explicit {
        Some(id) => find_profile(id).ok_or_else(|| unknown_platform(id)),
        None => Ok(detect(url)),
    }
}

/// Look up an image profile by its stable command id.
pub(crate) fn find_profile(id: &str) -> Option<&'static dyn ImageProfile> {
    REGISTRY
        .iter()
        .find(|entry| entry.profile.id() == id)
        .map(|entry| entry.profile)
}

/// Look up a catalog by its stable command id.
pub(crate) fn find_catalog(id: &str) -> Option<&'static dyn Catalog> {
    REGISTRY
        .iter()
        .find_map(|entry| entry.catalog.filter(|catalog| catalog.id() == id))
}

/// Whether an id names a platform that can be persisted by history, favorites
/// or downloads. Image-only profiles such as `generic` are not persistable.
pub(crate) fn is_known_platform(value: &str) -> bool {
    REGISTRY
        .iter()
        .any(|entry| entry.catalog.is_some() && entry.profile.id() == value)
}

/// Resolve a catalog, preserving the "platform does not support ..." error for
/// image-only profiles while unknown ids stay unknown.
pub(crate) fn catalog(id: &str, operation: &str) -> Result<&'static dyn Catalog> {
    find_catalog(id).ok_or_else(|| {
        if find_profile(id).is_some() {
            AppError::InvalidInput {
                field: "platform",
                reason: format!("platform `{id}` does not support {operation}"),
            }
        } else {
            unknown_platform(id)
        }
    })
}

fn unknown_platform(id: &str) -> AppError {
    AppError::InvalidInput {
        field: "platform",
        reason: format!("unknown platform `{id}`"),
    }
}

pub(crate) fn detect(url: &Url) -> &'static dyn ImageProfile {
    REGISTRY
        .iter()
        .map(|entry| entry.profile)
        .find(|profile| profile.matches(url))
        .unwrap_or(&GENERIC)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids = REGISTRY
            .iter()
            .map(|entry| entry.profile.id())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), REGISTRY.len());
    }

    #[test]
    fn recognizes_only_persistable_platforms() {
        for platform in ["manhuagui", "jm", "nhentai", "copymanga"] {
            assert!(is_known_platform(platform));
        }

        assert!(!is_known_platform(""));
        assert!(!is_known_platform("unknown"));
        assert!(!is_known_platform("generic"));
        assert!(!is_known_platform("NHENTAI"));
    }

    #[test]
    fn generic_is_an_image_profile_without_a_catalog() {
        assert!(find_profile("generic").is_some());
        assert!(find_catalog("generic").is_none());
    }

    #[test]
    fn exposes_platform_capabilities() {
        assert!(find_catalog("nhentai").unwrap().single_chapter());
        assert!(!find_catalog("jm").unwrap().single_chapter());
        assert!(!find_catalog("manhuagui").unwrap().single_chapter());
        assert!(!find_catalog("copymanga").unwrap().single_chapter());

        assert_eq!(
            find_catalog("jm")
                .unwrap()
                .normalize_work_id("JM123")
                .unwrap(),
            "123"
        );
        assert_eq!(
            find_catalog("nhentai")
                .unwrap()
                .normalize_work_id("0042")
                .unwrap(),
            "0042"
        );
        assert_eq!(
            find_catalog("copymanga")
                .unwrap()
                .normalize_work_id("lianaidaixing")
                .unwrap(),
            "lianaidaixing"
        );
        assert!(
            find_catalog("copymanga")
                .unwrap()
                .normalize_work_id("../bad")
                .is_err()
        );
    }
}
