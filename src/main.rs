use std::{collections::HashSet, env};

use axum::{routing, Router};
use color_eyre::eyre;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use tracing::warn;

mod logging;
mod operator;

/// Exclude namespaces that contain NAIS app services we don't care about.
///   Will:
///    - expect comma-separated string lists in environment variable names supplied
///    - remove duplicate namespaces
///    - returns comma-separated string of format `namespace!=<namespace name>`
fn collate_excluded_namespaces(env_vars: &[&str]) -> String {
	let excluded_namespaces: HashSet<String> = env_vars
		.iter()
		.flat_map(|env_var| {
			let Ok(env_val) = env::var(env_var) else {
				warn!("Unable to read supplied env var: {}", env_var);
				return HashSet::new();
			};
			if env_val.is_empty() {
				warn!("Supplied env var was empty: {}", env_var);
				return HashSet::new();
			}
			env_val
				.split(',')
				.filter(|s| !s.is_empty())
				.map(|ns| format!("namespace!={ns}"))
				.collect()
		})
		.collect();
	excluded_namespaces
		.into_iter()
		.collect::<Vec<_>>()
		.join(",")
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
	color_eyre::install()?;
	logging::init();

	// TODO: Sanitize log output which is too verbose
	// TODO: Allow operator to control is_ready status supplied by webserver
	// TODO: Switch which of the two runs in separate thread, so as to ensure all threads die if operator dies

	// Ensure port is available
	let socket = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;

	// Start k8s operator
	tokio::spawn(operator::run(&collate_excluded_namespaces(&[
		"PLATFORM_NAMESPACES",
	])));

	// Start webserver
	// TODO: Conside offering metrics/prometheus scraping endpoint
	let webapp = Router::new()
		.route("/health/ready", routing::get(|| async { todo!() }))
		.route("/health/alive", routing::get(|| async { todo!() }));
	axum::serve(socket, webapp.into_make_service())
		.await
		.map_err(eyre::Error::from)
}
