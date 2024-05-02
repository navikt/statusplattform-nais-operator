use std::{collections::HashSet, env};

use futures::TryStreamExt;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::{
	api::{Api, DynamicObject, GroupVersionKind, ResourceExt},
	runtime::{watcher, WatchStreamExt},
	Client,
};
use tracing::{debug, error, info, warn};

mod endpoint_slice;
use crate::endpoint_slice::{
	endpointslice_is_ready, extract_team_and_app_labels, has_service_owner,
};

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
async fn main() -> color_eyre::eyre::Result<()> {
	tracing_subscriber::fmt::init();

	// We want to filter:
	// - away resources w/o the labels we require
	// - away resources belonging to certain namespaces (TODO)
	let wc = watcher::Config::default().labels("app,team");
	// .fields(&collate_excluded_namespaces(&["PLATFORM_NAMESPACES"]));
	// .streaming_lists(); // TODO: Add back in when cluster supports WatchList feature

	let client = Client::try_default().await?;
	let nais_gvk = GroupVersionKind::gvk("nais.io", "v1alpha1", "Application");
	let (nais_crd, _api_caps) = kube::discovery::pinned_kind(&client, &nais_gvk).await?;

	watcher(Api::<EndpointSlice>::all(client.clone()), wc)
		.applied_objects()
		.default_backoff()
		.try_for_each(|endpoint_slice| {
			// Move these into scope of `.try_for_each()`'s async closure
			let client = client.clone();
			let nais_crd = nais_crd.clone();
			let nais_gvk = nais_gvk.clone();
			async move {
				let endpoint_slice_name = endpoint_slice.name_any();
				let Some(namespace) = endpoint_slice.namespace() else {
					// Something went horribly wrong when we cannot ascertain the namespace
					//   for the given EndpointSlice
					error!(%endpoint_slice_name, "Unable to ascertain namespace of endpoint_slice");
					return Ok(());
				};
				info!(%namespace, %endpoint_slice_name, "Starting to look at endpoint");

				let (app_name, team_name) = match extract_team_and_app_labels(&endpoint_slice) {
					(Some(app), Some(team)) => (app, team),
					(app, team) => {
						if app.is_none() {warn!(%namespace, ?endpoint_slice, "Unable to find `app` label on EndpointSlice");}
						if team.is_none() {warn!(%namespace, ?endpoint_slice, "Unable to find `team` label on EndpointSlice");}
						return Ok(());
					},
				};
				if namespace != team_name {
					warn!(%team_name, %namespace, "`team_name` label does not match namespace");
					// TODO: Decide if we care enough to do anything about this
				}

				debug!(%team_name, %app_name, "Checking owner reference(s)");
				let has_expected_owner = has_service_owner(&endpoint_slice, &app_name);
				if !has_expected_owner {
					// This is not an endpoint generated for a service, we should not care.
					return Ok(());
				};

				// Ensure owner reference to a nais.io/XXXX Application
				let nais_apps = Api::<DynamicObject>::namespaced_with(client, &namespace, &nais_crd);
				match nais_apps.get_opt(&app_name).await {
					Err(e) => {
						error!(?nais_gvk, %namespace, ?e, "Error occurred when attempting to fetch nais app");
						// return Err(e); // TODO: Fix this so backoff can handle it
						return Ok(());
					},
					Ok(nais_app) => {
						let Some(_) = nais_app else {
							warn!(?nais_gvk, %namespace, "Unable to find any NAIS app");
							return Ok(());
						};
					},
				};

				info!(%namespace, %app_name, %endpoint_slice_name, "Ascertained that this EndpointSlice seems to be a product of a NAIS app");
				if endpointslice_is_ready(&endpoint_slice) {
					warn!(%namespace, %app_name, "Nais app is alive!!!");
				} else {
					warn!(%namespace, %app_name, "Nais app is dead!!!");
				}
				// TODO: Send http request to the statusplattform backend API w/reqwest
				todo!();
				// Ok(()) // TODO: Comment back in when removing above todo!()
			}
		})
		.await?;
	Ok(())
}
