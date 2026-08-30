use aidoku::{
	alloc::{String, Vec},
	imports::defaults::defaults_get,
};

const MIRROR_DOMAIN_KEY: &str = "mirrorDomain";
const BLOCKED_CONTENT_KEY: &str = "blockedMetadataKeywords";

pub fn mirror_domain() -> Option<String> {
	defaults_get::<String>(MIRROR_DOMAIN_KEY).filter(|value| !value.trim().is_empty())
}

pub fn blocked_entries() -> Vec<String> {
	defaults_get::<Vec<String>>(BLOCKED_CONTENT_KEY).unwrap_or_default()
		.into_iter()
		.map(|value| value.trim().to_lowercase())
		.filter(|value| !value.is_empty())
		.collect()
}
