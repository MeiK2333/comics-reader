//! Platform ids shared by the persisted stores.
//!
//! History, favorites and downloads all validate the same set of ids. Keeping
//! it in one place means adding a platform cannot leave one store behind.

pub(crate) const KNOWN_PLATFORMS: [&str; 3] = ["manhuagui", "jm", "nhentai"];

pub(crate) fn is_known_platform(value: &str) -> bool {
    KNOWN_PLATFORMS.contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_the_supported_platforms() {
        for platform in KNOWN_PLATFORMS {
            assert!(is_known_platform(platform));
        }

        assert!(!is_known_platform(""));
        assert!(!is_known_platform("unknown"));
        assert!(!is_known_platform("NHENTAI"));
    }
}
