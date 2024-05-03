use std::{collections::HashSet, env};

use color_eyre::eyre;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use tracing::warn;

mod operator;

// TODO: Remove clippy exception once we figure out how to perform the filtering
#[allow(dead_code)]
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
	tracing_subscriber::fmt::init();

	operator::run(&collate_excluded_namespaces(&["PLATFORM_NAMESPACES"])).await?;
	Ok(())
}
