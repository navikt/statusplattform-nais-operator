use std::env;

#[derive(Clone, Debug)]
pub struct Config {
	pub api_key: String,
	pub excluded_namespaces: String,
	pub base_url: String,
}

pub fn new() -> Config {
	#[allow(clippy::expect_used)]
	// We want to crash loop in kaputtkonfiguriert environments
	let api_key = env::var("swagger-api-key").expect("api key in env");
	let excluded_namespaces = env::var("PLATFORM_NAMESPACES").unwrap_or_else(|_| String::new());
	let base_url = env::var("BASE_URL").unwrap_or_else(|_| String::from("http://portalserver"));
	Config {
		api_key,
		excluded_namespaces,
		base_url,
	}
}
