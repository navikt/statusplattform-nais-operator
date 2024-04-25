use std::{collections::HashSet, env};

use futures::TryStreamExt;
use k8s_openapi::{api::discovery::v1::EndpointSlice, Metadata};
use kube::{
	api::{Api, ObjectMeta, ResourceExt},
	runtime::{watcher, WatchStreamExt},
	Client,
};
use tracing::{info, warn};

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
				return Vec::new();
			};
			env_val
				.split(',')
				.map(|ns| format!("namespace!={ns}"))
				.collect()
		})
		.collect();
	excluded_namespaces
		.into_iter()
		.collect::<Vec<String>>()
		.join(",")
}

/// Returns true if and only if at least one of the `EndpointSlice`'s
///  `endpoints[].conditions.ready` evaluate to `true`
///  Always return false elsewise.
fn endpointslice_is_ready(endpoint_slice: &EndpointSlice) -> bool {
	endpoint_slice
		.endpoints
		.iter()
		.flat_map(|e| &e.conditions)
		.filter_map(|c| c.ready)
		.any(|is_ready| is_ready)
}

fn has_expected_owner_reference(o: &ObjectMeta, app_name: Option<&str>) -> bool {
	let Some(name) = app_name else {
		// We expect any `EndpointSlice`s made by `Service`s to propagate the `Service`'s labels
		return false;
	};
	let Some(ref owners) = o.owner_references else {
		// We only care about `EndpointSlice`s that've owner references to a `Service`
		return false;
	};
	owners
		.iter()
		.any(|o| o.api_version == "v1" && o.kind == "Service" && o.name.as_str() == name)
}

#[tokio::main]
async fn main() -> color_eyre::eyre::Result<()> {
	tracing_subscriber::fmt::init();
	// requires WatchList feature gate on 1.27 or later: TODO check if cluster supports
	let wc = watcher::Config::default()
        .labels("app,team") // I just care if the label(s) exist
        .fields(&collate_excluded_namespaces(&["PLATFORM_NAMESPACES"]))
        .streaming_lists();

	watcher(Api::<EndpointSlice>::all(Client::try_default().await?), wc)
		.applied_objects()
		.default_backoff()
		.try_for_each(|s| async move {
			info!("saw {}", s.name_any());

			let has_expected_owner = has_expected_owner_reference(
				s.metadata(),
				s.labels().get("app").map(String::as_str),
			);
			if !has_expected_owner {
				// This is not an endpoint generated for a service, we should not care.
				return Ok(());
			};

			// s.metadata... // TODO: Ensure owner reference to a nais.io/XXXX Application

			if endpointslice_is_ready(&s) {
				// TODO: Send http request to the statusplattform backend API w/reqwest
				warn!(
					"{}/{} is alive!!!",
					s.metadata.namespace.unwrap(),
					s.metadata.name.unwrap()
				);
			} else {
				warn!(
					"{}/{} is dead!!!",
					s.metadata.namespace.unwrap(),
					s.metadata.name.unwrap()
				);
			}
			// TODO: Consider/research if the events can be further filtered to only send http request to backend
			todo!();
			// Ok(()) // TODO: Comment back in when removing above todo!()
		})
		.await?;
	Ok(())
}
