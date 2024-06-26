//! This module contains the logic specific to this k8s operator's execution/main-loop

use color_eyre::eyre;
use futures::{Future, TryStreamExt};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::{
	api::{ApiResource, DynamicObject, GroupVersionKind},
	runtime::{watcher, WatchStreamExt},
	Api, Client, ResourceExt,
};
use tracing::{debug, error, info, warn, Span};

mod endpoint_slice;
use crate::operator::endpoint_slice::{
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

/// The "inner"/"hot loop" of this k8s operator.
/// In other words, where the important shit happens.
///
/// It looks at each `EndpointSlice`, and
/// 1. Finds metadata re. the `EndpointSlice`
/// 1. Uses the metadata to try and eliminate `EndpointSlices` we don't care about
/// 1. Reports back to statusplattform-backend w/HTTP request whether a NAIS app's Service's `EndpointSlice` reports if the app's readiness probes say its ready for traffic or not.
///
/// # Errors
///
/// This function will return an error if it encounters a situation we believe should never happen.
async fn endpoint_slice_handler(
	endpoint_slice: EndpointSlice,
	client: Client,
	nais_crds: ApiResource,
	nais_gvk: &GroupVersionKind,
	parent_span: &Span,
) -> eyre::Result<()> {
	Span::current().follows_from(parent_span);
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

	let has_expected_owner = has_service_owner(&endpoint_slice, &app_name, &Span::current());
	if !has_expected_owner {
		warn!("EndpointSlice does not have expected Service owner reference");
		return Ok(());
	};
	debug!("Found expected Service owner-reference to EndpointSlice");

	if get_nais_app(
		Api::<DynamicObject>::namespaced_with(client, &namespace, &nais_crds)
			.get_opt(&app_name)
			.await,
		nais_gvk,
		&Span::current(),
	)
	.is_none()
	{
		warn!("Unable to find any expected NAIS app");
		return Ok(());
	};
	info!("Found NAIS app that seems to match this EndpointSlice");

	// TODO: We explicit create a new client, use it and discard it. Expensive.
	let client = reqwest::Client::new();
	if endpointslice_is_ready(&endpoint_slice) {
		let res = client
			.post("https://portal-server")
			.body("the exact body that is sent")
			.send()
			.await?;
	} else {
		let res = client
			.post("https://portal-server")
			.body("the exact body that is sent")
			.send()
			.await?;
	}
	Ok(())
}

/// Starts the (ideally eternally running) `kube::runtime::watcher` with the supplied variables required.
///
/// # Errors
///
/// This function will return an error if the watcher returns an error it cannot recover from.
fn init(
	client: Client,
	nais_apps: ApiResource,
	nais_gvk: GroupVersionKind,
	main_span: Span,
	wc: watcher::Config,
) -> impl Future<Output = eyre::Result<()>> {
	watcher(Api::<EndpointSlice>::all(client.clone()), wc)
		.applied_objects()
		.default_backoff()
		.map_err(eyre::Error::from)
		.try_for_each(move |endpoint_slice| {
			// Move these into scope of `.try_for_each()`'s async closure
			let client = client.clone();
			let nais_apps = nais_apps.clone();
			let nais_gvk = nais_gvk.clone();

			// Leverage tracing module's `Span`s logging functionality
			let outer_loop_log_span = Span::current();
			outer_loop_log_span.follows_from(&main_span);
			outer_loop_log_span.record("endpoint_slice_name", &endpoint_slice.name_any());
			async move {
				endpoint_slice_handler(
					endpoint_slice,
					client,
					nais_apps,
					&nais_gvk,
					&outer_loop_log_span,
				)
				.await
			}
		})
}

// TODO: Remove when namespaces are filtered
#[allow(unused_variables)]
/// Sets up, starts, and runs an (eternally running) `kube::runtime::watcher`
///
/// # Errors
///
/// This function will return an error if the watcher returns an error it cannot backoff retry from.
pub fn run(excluded_namespaces: &str) -> impl Future<Output = eyre::Result<()>> {
	// We want to filter:
	// - away resources w/o the labels we require
	// - away resources belonging to certain namespaces (TODO)
	let wc = watcher::Config::default().labels("app,team");
	// .fields(&_excluded_namespaces);
	// .streaming_lists(); // TODO: Add back in when cluster supports WatchList feature

	let nais_gvk = GroupVersionKind::gvk("nais.io", "v1alpha1", "Application");
	let main_span = Span::current();

	async move {
		let client = Client::try_default().await?;
		let (nais_crd, _) = kube::discovery::pinned_kind(&client, &nais_gvk).await?;
		init(client, nais_crd, nais_gvk, main_span, wc).await
	}
}
