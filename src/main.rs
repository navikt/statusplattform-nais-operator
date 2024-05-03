use std::{collections::HashSet, env};

use color_eyre::eyre;
use futures::TryStreamExt;
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::{
	api::{Api, DynamicObject, GroupVersionKind, ResourceExt},
	runtime::{watcher, WatchStreamExt},
	Client,
};
use tracing::{debug, error, info, warn, Span};

mod endpoint_slice;
use crate::endpoint_slice::{
	endpointslice_is_ready, extract_team_and_app_labels, has_service_owner,
};

/// Each (interesting to us) `EndpointSlice` is expected to
///  - have a matching NAIS app
///    - of the same name as the `EndpointSlice`'s `app` label
///  This function returns true if and only if such a NAIS app is found in the
///   same namespace as the `EndpointSlice`
fn get_nais_app(
	nais_app: Result<Option<DynamicObject>, kube::Error>,
	nais_gvk: &GroupVersionKind,
	parent_span: &tracing::Span,
) -> Option<DynamicObject> {
	Span::current().follows_from(parent_span);
	match nais_app {
		Err(e) => {
			error!(
				?nais_gvk,
				?e,
				"Error occurred when attempting to fetch NAIS app"
			);
			None
		},
		Ok(found_app) => found_app,
	}
}

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

	// We want to filter:
	// - away resources w/o the labels we require
	// - away resources belonging to certain namespaces (TODO)
	let wc = watcher::Config::default().labels("app,team");
	// .fields(&collate_excluded_namespaces(&["PLATFORM_NAMESPACES"]));
	// .streaming_lists(); // TODO: Add back in when cluster supports WatchList feature

	let client = Client::try_default().await?;
	let nais_gvk = GroupVersionKind::gvk("nais.io", "v1alpha1", "Application");
	let (nais_crd, _api_caps) = kube::discovery::pinned_kind(&client, &nais_gvk).await?;
	let main_span = Span::current();

	watcher(Api::<EndpointSlice>::all(client.clone()), wc)
		.applied_objects()
		.default_backoff()
		.map_err(eyre::Error::from)
		.try_for_each(|endpoint_slice| {
			// Move these into scope of `.try_for_each()`'s async closure
			let client = client.clone();
			let nais_crd = nais_crd.clone();
			let nais_gvk = nais_gvk.clone();

			// Leverage tracing module's `Span`s logging functionality
			let outer_loop_log_span = Span::current();
			outer_loop_log_span.follows_from(&main_span);
			outer_loop_log_span.record("endpoint_slice_name", &endpoint_slice.name_any());
			async move {
				Span::current().follows_from(outer_loop_log_span);
				info!("Starting to look at endpoint");

				let Some(namespace) = endpoint_slice.namespace() else {
					// All `EndpointSlice`s should belong to a namespace...
					error!("Unable to ascertain namespace of EndpointSlice");
					return Ok(());
				};
				Span::current().record("namespace", &namespace);
				debug!("Ascertained namespace of EndpointSlice");

				let Some((app_name, team_name)) =
					extract_team_and_app_labels(&endpoint_slice, &Span::current())
				else {
					warn!("Unable to fetch required labels on EndpointSlice");
					return Ok(());
				};
				Span::current().record("app_name", &app_name);
				Span::current().record("team_name", &team_name);
				debug!("Found required labels on EndpointSlice");

				if namespace != team_name {
					warn!("`team` label does not match namespace");
					// TODO: Decide if we care enough to do anything about this
				}

				let has_expected_owner =
					has_service_owner(&endpoint_slice, &app_name, &Span::current());
				if !has_expected_owner {
					warn!("EndpointSlice does not have expected Service owner reference");
					return Ok(());
				};
				debug!("Found expected Service owner-reference to EndpointSlice");

				if get_nais_app(
					Api::<DynamicObject>::namespaced_with(client, &namespace, &nais_crd)
						.get_opt(&app_name)
						.await,
					&nais_gvk,
					&Span::current(),
				)
				.is_none()
				{
					warn!("Unable to find any expected NAIS app");
					return Ok(());
				};
				info!("Found NAIS app that seems to match this EndpointSlice");

				if endpointslice_is_ready(&endpoint_slice) {
					warn!("Nais app is alive!!!");
				} else {
					warn!("Nais app is dead!!!");
				}
				// TODO: Send http request to the statusplattform backend API w/reqwest
				todo!();
				// Ok(()) // TODO: Comment back in when removing above todo!()
			}
		})
		.await?;
	Ok(())
}
