use std::io::IsTerminal;

use tracing::Level;
use tracing_subscriber::{filter, layer::SubscriberExt, util::SubscriberInitExt, Registry};

pub fn init() {
	use tracing_subscriber::fmt as layer_fmt;
	let (plain_log_format, json_log_format) = if std::io::stdout().is_terminal() {
		(Some(layer_fmt::layer().compact()), None)
	} else {
		(None, Some(layer_fmt::layer().json().flatten_event(true)))
	};

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
