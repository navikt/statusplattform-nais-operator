//! This module contains `k8s_openapi::api::discovery::v1::EndpointSlice` utility functions
use crate::EndpointSlice;

use k8s_openapi::Metadata;
use kube::ResourceExt;
use tracing::{error, warn, Span};

/// Returns true if and only if at least one of the `EndpointSlice`'s
///  `endpoints[].conditions.ready` evaluate to `true`
///  Always return false elsewise.
pub fn endpointslice_is_ready(endpoint_slice: &EndpointSlice) -> bool {
	endpoint_slice
		.endpoints
		.iter()
		.flat_map(|e| &e.conditions)
		.filter_map(|c| c.ready)
		.any(|is_ready| is_ready)
}

/// Returns true if and only if the `EndpointSlice`'s `.metadata()`
///  refers to a `k8s_openapi::api::core::v1::Service` of the same name as the `app` label.
///  Always returns false elsewise.
pub fn has_service_owner(
	endpoint_slice: &EndpointSlice,
	app_name: &str,
	parent_span: &tracing::Span,
) -> bool {
	Span::current().follows_from(parent_span);

	let Some(ref owners) = endpoint_slice.metadata().owner_references else {
		// We only care about `EndpointSlice`s that've owner references to a `Service`
		error!("EndpointSlice lacks owner reference");
		return false;
	};

	let owner_is_a_service = owners
		.iter()
		.any(|o| o.api_version == "v1" && o.kind == "Service" && o.name.as_str() == app_name);
	if !owner_is_a_service {
		error!("EndpointSlice has no Service owner");
	}

	owner_is_a_service
}

/// Helper function to get the `team` and `app` labels from a K8s resource
pub fn extract_team_and_app_labels(
	endpoint_slice: &EndpointSlice,
	parent_span: &tracing::Span,
) -> Option<(String, String)> {
	Span::current().follows_from(parent_span);

	// We expect the:
	//   - team's given app name
	//   - team's name
	//  to be present in this label.
	// As is customary for things generated by naiserator/NAIS apps
	let (app, team) = match (
		endpoint_slice.labels().get("app"),
		endpoint_slice.labels().get("team"),
	) {
		(Some(app), Some(team)) => (app, team),
		(app, team) => {
			if app.is_none() {
				warn!("Unable to find `app` label on EndpointSlice");
			}
			if team.is_none() {
				warn!("Unable to find `team` label on EndpointSlice");
			}
			error!("Missing required label(s)");
			return None;
		},
	};

	Some((app.to_owned(), team.to_owned()))
}
