use crate::config;
use color_eyre::eyre;
use reqwest;

pub struct Client {
	client: reqwest::Client,
	base_url: String,
}

impl Client {
	fn new(client: reqwest::Client, base_url: String) -> Self {
		Self { client, base_url }
	}

	pub fn get(&self, endpoint: &str) -> reqwest::RequestBuilder {
		self.client.get(format!("{}/{}", self.base_url, endpoint))
	}

	pub fn post(&self, endpoint: &str) -> reqwest::RequestBuilder {
		self.client.post(format!("{}/{}", self.base_url, endpoint))
	}

	pub fn put(&self, endpoint: &str) -> reqwest::RequestBuilder {
		self.client.put(format!("{}/{}", self.base_url, endpoint))
	}
}

pub(crate) fn new(config: &config::Config) -> eyre::Result<Client> {
	let Ok(header) = reqwest::header::HeaderValue::from_str(&config.api_key) else {
		// TODO: This is clearly incorrect, a real error please
		panic!()
	};

	let mut headers = reqwest::header::HeaderMap::new();
	headers.insert("Apikey", header);

	let client = reqwest::Client::builder()
		.default_headers(headers)
		.build()?;
	let conf = config.clone();
	Ok(Client::new(client, conf.base_url))
}
