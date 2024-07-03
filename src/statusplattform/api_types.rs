use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct RecordDto {
	pub service_id: uuid::Uuid,
	pub status: StatusDto,
	pub source: RecordSourceDto,
	pub description: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RecordSourceDto {
	#[serde(rename(serialize = "GCP_POLL"))]
	GcpPoll,
	// OnPremPoll, -- these never exist to us since we only do gcp
	// Prometheus,
	// Obm,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum StatusDto {
	// these have weird capitalization because the other end is weird.
	#[serde(rename(serialize = "OK"))]
	Ok,
	#[serde(rename(serialize = "DOWN"))]
	Down,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum AreaDto {
	// We don't care to implement this at time of writing
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServiceDto {
	pub name: String,
	#[serde(rename(serialize = "type"))]
	pub typ: String,
	//	service_type: Option<ServiceTypeDto>,
	pub team: String,
	//	pub team_id: Option<uuid::Uuid>,
	pub service_dependencies: Vec<ServiceDto>,
	pub component_dependencies: Vec<ServiceDto>,
	pub areas_containing_this_service: Vec<AreaDto>,
	pub services_dependent_on_this_component: Vec<ServiceDto>,
	//	oh_display: OHdisplayDto,
	//	monitorlink: String,
	//	polling_url: String,
	// record: RecordDto,
}
