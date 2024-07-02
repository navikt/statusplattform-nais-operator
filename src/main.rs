use std::{collections::HashSet, env};

use axum::{routing, Router};
use color_eyre::eyre;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use reqwest::StatusCode;
use tracing::warn;

mod config;
mod logging;
mod operator;
pub mod statusportal;
/// Exclude namespaces that contain NAIS app services we don't care about.
///   Will:
///    - expect comma-separated string lists in environment variable names supplied
///    - remove duplicate namespaces
///    - returns comma-separated string of format `namespace!=<namespace name>`
fn collate_excluded_namespaces(env_vars: &config::Config) -> String {
	if env_vars.excluded_namespaces.is_empty() {
		return "".to_string();
	}

	let excluded_namespaces: HashSet<String> = env_vars
		.excluded_namespaces
		.split(',')
		.filter(|s| !s.is_empty())
		.map(|ns| format!("namespace!={ns}"))
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
	let config = config::new()?;
	let (ready_tx, ready_rx) = tokio::sync::watch::channel(true);

	// Ensure port is available
	let socket = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
	// Start webserver in sibling thread, so that if the main thread "dies"/stops, this gets cleaned up
	tokio::spawn(async {
		axum::serve(
			socket,
			Router::new()
				.route(
					"/health/ready",
					axum::routing::get(move || async move {
						if *ready_rx.borrow() {
							StatusCode::OK
						} else {
							StatusCode::SERVICE_UNAVAILABLE
						}
					}),
				)
				.route("/health/alive", routing::get(|| async { "I'm alive!" }))
				.into_make_service(),
		)
	});

	// Start k8s operator
	operator::run(&collate_excluded_namespaces(&config), &config, ready_tx).await
	// futures::future::pending::<()>().await; // Functionally/spiritually equivalent of above line
}
