//! This module contains the logic specific to this k8s operator's execution/main-loop

use std::collections::HashMap;

use color_eyre::eyre::{self, OptionExt};
use futures::{Future, TryStreamExt};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::{
	api::{ApiResource, DynamicObject, GroupVersionKind},
	runtime::{watcher, WatchStreamExt},
	Api, Client, ResourceExt,
};
use serde::Deserialize;
use tracing::{debug, error, info, warn, Span};
use uuid::Uuid;

mod endpoint_slice;
mod java_dto;

use crate::statusportal;
use crate::{
	config,
	operator::endpoint_slice::{
		endpointslice_is_ready, extract_team_and_app_labels, has_service_owner,
	},
};

type ServiceId = Uuid;
type ServiceName = String;
#[derive(Deserialize, Debug, Clone)]
struct ServiceJson {
	name: ServiceName,
	id: ServiceId,
	team_id: Uuid,
}

// TODO: Remove when namespaces are filtered
#[allow(unused_variables)]
/// Sets up, starts, and runs an (eternally running) `kube::runtime::watcher`
///
/// # Errors
///
/// This function will return an error if the watcher returns an error it cannot backoff retry from.

pub fn run<'a>(
	excluded_namespaces: &'a str,
	config: &'a config::Config,
	ready_tx: tokio::sync::watch::Sender<bool>,
) -> impl Future<Output = eyre::Result<()>> + 'a {
	// We want to filter:
	// - away resources w/o the labels we require
	// - away resources belonging to certain namespaces (TODO)
	let wc = watcher::Config::default().labels("app,team");
	// .fields(&_excluded_namespaces);
	// .streaming_lists(); // TODO: Add back in when cluster supports WatchList feature

	let nais_gvk = GroupVersionKind::gvk("nais.io", "v1alpha1", "Application");
	let main_span = Span::current();

	async move {
		let client = match Client::try_default().await {
			Ok(client) => {
				//	if let Err(e) = ready_tx.send(true) {
				//		return Err(eyre::eyre!("Failed to send ready signal: {:?}", e));
				//	}
				client
			},
			Err(e) => {
				error!("Failed to create client: {:?}", e);
				return Err(eyre::eyre!("Failed to create client: {:?}", e));
			},
		};

		let (nais_crd, _) = kube::discovery::pinned_kind(&client, &nais_gvk).await?;
		init(config, client, nais_crd, nais_gvk, main_span, wc).await
	}
}

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
	portal_client: statusportal::Client,
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

	let service_id = match portal_client
		.get("rest/Services")
		.send()
		.await?
		.json::<Vec<ServiceJson>>()
		.await?
		.into_iter()
		.map(|e| (e.name, e.id))
		.collect::<HashMap<ServiceName, ServiceId>>()
		.get(&app_name)
	{
		Some(service) => service.to_owned(),
		None => {
			let apps = portal_client
				.post("rest/Service")
				.json(&java_dto::ServiceDto {
					name: app_name.clone(),
					team: namespace,
					team_id: None,
					service_dependencies: Vec::new(),
					component_dependencies: Vec::new(),
					areas_containing_this_service: Vec::new(),
					services_dependent_on_this_component: Vec::new(),
				})
				.send()
				.await?
				.error_for_status()
				.map_err(eyre::Error::from)?
				.json::<Vec<ServiceJson>>()
				.await?
				.into_iter()
				.map(|e| (e.name, e.id))
				.collect::<HashMap<ServiceName, ServiceId>>();
			*apps.get(&app_name).unwrap()
		},
	};

	let body = java_dto::RecordDto {
		service_id,
		status: if endpointslice_is_ready(&endpoint_slice) {
			java_dto::StatusDto::OK
		} else {
			java_dto::StatusDto::DOWN
		},
		source: java_dto::RecordSourceDto::GcpPoll,
		description: format!("Status sent from {}", env!("CARGO_PKG_NAME")),
	};
	portal_client
		.put("rest/ServiceStatus")
		.json(&body)
		.send()
		.await?
		.error_for_status()
		.map_err(eyre::Error::from)
		.map(|_| ()) // We don't care about successful return value(s)
}

/// Starts the (ideally eternally running) `kube::runtime::watcher` with the supplied variables required.
///
/// # Errors
///
/// This function will return an error if the watcher returns an error it cannot recover from.
fn init(
	config: &config::Config,
	client: Client,
	nais_apps: ApiResource,
	nais_gvk: GroupVersionKind,
	main_span: Span,
	wc: watcher::Config,
) -> impl Future<Output = eyre::Result<()>> + '_ {
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

			let portal_client = statusportal::new(&config);

			async move {
				endpoint_slice_handler(
					endpoint_slice,
					client,
					portal_client?,
					nais_apps,
					&nais_gvk,
					&outer_loop_log_span,
				)
				.await
			}
		})
}
