use crate::config;
use color_eyre::eyre;

pub struct Client {
	client: reqwest::Client,
	base_url: String,
}

pub mod api_types;

impl Client {
	const fn new(client: reqwest::Client, base_url: String) -> Self {
		Self { client, base_url }
	}

	pub fn get(&self, endpoint: &str) -> reqwest::RequestBuilder {
		self.client.get(format!("{}/{}", self.base_url, endpoint))
	}

	pub fn post(&self, endpoint: &str) -> reqwest::RequestBuilder {
		self.client.post(format!("{}/{}", self.base_url, endpoint))
	}
}

pub fn new(config: &config::Config) -> eyre::Result<Client> {
	let Ok(header) = reqwest::header::HeaderValue::from_str(&config.api_key) else {
		eyre::bail!("Failed at constructing the api key header")
	};

	let mut headers = reqwest::header::HeaderMap::new();
	headers.insert("Apikey", header);

	let client = reqwest::Client::builder()
		.default_headers(headers)
		.build()?;
	let conf = config.clone();
	Ok(Client::new(client, conf.base_url))
}
