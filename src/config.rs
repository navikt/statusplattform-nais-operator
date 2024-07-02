use std::env;

use color_eyre::eyre;

#[derive(Clone)]
pub(crate) struct Config {
	pub api_key: String,
	pub excluded_namespaces: String,
	pub base_url: String,
}

pub(crate) fn new() -> eyre::Result<Config> {
	let api_key = env::var("swagger-api-key")?;
	let excluded_namespaces = env::var("PLATFORM_NAMESPACES")?;
	let base_url = env::var("BASE_URL").unwrap_or("https://statusplatform".into());
	Ok(Config {
		api_key,
		excluded_namespaces,
		base_url,
	})
}
