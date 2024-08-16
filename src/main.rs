use std::collections::HashSet;

use axum::{routing, Router};
use color_eyre::eyre;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use reqwest::StatusCode;

use std::io::IsTerminal;
use tracing_opentelemetry::OpenTelemetryLayer;

use opentelemetry::{global, trace::TracerProvider, KeyValue};
use opentelemetry_sdk::{
	runtime,
	trace::{BatchConfig, RandomIdGenerator, Sampler, Tracer},
	Resource,
};
use opentelemetry_semantic_conventions::{
	resource::{DEPLOYMENT_ENVIRONMENT, SERVICE_NAME, SERVICE_VERSION},
	SCHEMA_URL,
};
use tracing::Level;
use tracing_subscriber::{filter, layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod operator;
mod statusplattform;
/// Exclude namespaces that contain NAIS app services we don't care about.
///   Will:
///    - expect comma-separated string lists in environment variable names supplied
///    - remove duplicate namespaces
///    - returns comma-separated string of format `namespace!=<namespace name>`
fn collate_excluded_namespaces(env_vars: &config::Config) -> String {
	if env_vars.excluded_namespaces.is_empty() {
		return String::new();
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

	let config = config::new();
	let (ready_tx, ready_rx) = tokio::sync::watch::channel(true);

	use tracing_subscriber::fmt as layer_fmt;
	let (plain_log_format, json_log_format) = if std::io::stdout().is_terminal() {
		(Some(layer_fmt::layer().compact()), None)
	} else {
		(None, Some(layer_fmt::layer().json().flatten_event(true)))
	};

	// Create an OTLP pipeline exporter for a `trace_demo` service.

	let tracer = init_tracer();

	tracing_subscriber::registry()
		.with(tracing_subscriber::filter::LevelFilter::from_level(
			Level::INFO,
		))
		.with(tracing_subscriber::fmt::layer())
		.with(OpenTelemetryLayer::new(tracer))
		.with(plain_log_format)
		.with(json_log_format)
		.with(
			filter::Targets::new()
				.with_default(Level::INFO)
				.with_target("axum::rejection", Level::TRACE)
				.with_target("hyper", Level::ERROR)
				.with_target("kube_client", Level::ERROR)
				.with_target("hyper_util", Level::ERROR)
				.with_target("reqwest", Level::ERROR)
				.with_target("tower", Level::ERROR),
		)
		.init();

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

fn resource() -> Resource {
	Resource::from_schema_url(
		[
			KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
			KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
			KeyValue::new(DEPLOYMENT_ENVIRONMENT, "develop"),
		],
		SCHEMA_URL,
	)
}

fn init_tracer() -> Tracer {
	let provider = opentelemetry_otlp::new_pipeline()
		.tracing()
		.with_trace_config(
			opentelemetry_sdk::trace::Config::default()
                // Customize sampling strategy
                .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                    1.0,
                ))))
                // If export trace to AWS X-Ray, you can use XrayIdGenerator
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(resource()),
		)
		.with_batch_config(BatchConfig::default())
		.with_exporter(opentelemetry_otlp::new_exporter().tonic())
		.install_batch(runtime::Tokio)
		.unwrap();

	global::set_tracer_provider(provider.clone());
	provider.tracer("tracing-otel-subscriber")
}
