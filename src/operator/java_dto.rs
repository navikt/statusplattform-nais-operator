use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct RecordDto {
	// ^ sic! that's the name elsewhere
	id: uuid::Uuid,
	service_id: uuid::Uuid,
	status: StatusDto,
	source: RecordSourceDto,
	description: String,
}

#[derive(Serialize, Deserialize)]
enum RecordSourceDto {
	#[serde(rename(serialize = "GCP_POLL", deserialize = "GCP_POLL"))]
	GcpPoll,
	// OnPremPoll, -- these never exist to us since we only do gcp
	// Prometheus,
	// Obm,
}

#[derive(Serialize, Deserialize)]
enum StatusDto {
	// these have weird capitalization because the other end is weird.
	OK,
	DOWN,
}
enum AreaDto {}

struct ServiceDto {
	name: String,
	id: uuid::Uuid,
	//	service_type: Option<ServiceTypeDto>,
	team: String,
	team_id: uuid::Uuid,
	//	service_dependencies: Vec<ServiceDto>,
	//	component_dependencies: Vec<ServiceDto>,
	//	areas_containing_this_service: Vec<AreaDto>,
	//	services_dependent_on_this_component: Vec<ServiceDto>,
	//	oh_display: OHdisplayDto,
	//	monitorlink: String,
	//	polling_url: String,
	polling_on_prem: bool,
	record: RecordDto,
	status_not_from_team: bool,
}
