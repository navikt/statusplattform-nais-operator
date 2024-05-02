use std::io::IsTerminal;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};

pub fn init() {
	use tracing_subscriber::fmt as layer_fmt;
	let (plain_log_format, json_log_format) = if std::io::stdout().is_terminal() {
		(Some(layer_fmt::layer().compact()), None)
	} else {
		(None, Some(layer_fmt::layer().json().flatten_event(true)))
	};
	Registry::default()
		// .with(otel_layer) // TODO
		.with(plain_log_format)
		.with(json_log_format)
		.init();
}
