use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct RecordDto {
	pub service_id: uuid::Uuid,
	pub status: StatusDto,
	pub source: RecordSourceDto,
	pub description: String,
}

#[derive(Serialize, Deserialize)]
pub enum RecordSourceDto {
	#[serde(rename(serialize = "GCP_POLL"))]
	GcpPoll,
	// OnPremPoll, -- these never exist to us since we only do gcp
	// Prometheus,
	// Obm,
}

#[derive(Serialize, Deserialize)]
pub enum StatusDto {
	// these have weird capitalization because the other end is weird.
	OK,
	DOWN,
}

#[derive(Serialize, Deserialize)]
pub(crate) enum AreaDto {
	// We don't care to implement this at time of writing
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ServiceDto {
	pub(crate) name: String,
	//	service_type: Option<ServiceTypeDto>,
	pub(crate) team: String,
	pub(crate) team_id: uuid::Uuid,
	pub(crate) service_dependencies: Vec<ServiceDto>,
	pub(crate) component_dependencies: Vec<ServiceDto>,
	pub(crate) areas_containing_this_service: Vec<AreaDto>,
	pub(crate) services_dependent_on_this_component: Vec<ServiceDto>,
	//	oh_display: OHdisplayDto,
	//	monitorlink: String,
	//	polling_url: String,
	// record: RecordDto,
}
