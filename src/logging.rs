use std::io::IsTerminal;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::Tracer;
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_stdout as stdout;
use tracing::Level;
use tracing::{error, span};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{filter, layer::SubscriberExt, util::SubscriberInitExt, Registry};

pub fn init() {
	use tracing_subscriber::fmt as layer_fmt;
	let (plain_log_format, json_log_format) = if std::io::stdout().is_terminal() {
		(Some(layer_fmt::layer().compact()), None)
	} else {
		(None, Some(layer_fmt::layer().json().flatten_event(true)))
	};
	// Create a new OpenTelemetry trace pipeline that prints to stdout
	let provider = TracerProvider::builder()
		.with_simple_exporter(stdout::SpanExporter::default())
		.build();
	let tracer = provider.tracer("readme_example");

	// Create a tracing layer with the configured tracer
	let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

	// Use the tracing subscriber `Registry`, or any other subscriber
	// that impls `LookupSpan`
	let subscriber = Registry::default().with(telemetry);

	Registry::default() // TODO: .with(otel_layer)
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
}
