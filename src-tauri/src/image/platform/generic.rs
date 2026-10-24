use reqwest::RequestBuilder;
use url::Url;

use super::Platform;

/// Fallback for hosts without a dedicated profile: browser headers only.
pub(super) struct Generic;

impl Platform for Generic {
    fn id(&self) -> &'static str {
        "generic"
    }

    fn configure(&self, request: RequestBuilder, _url: &Url) -> RequestBuilder {
        request
    }
}
