use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct JavaDto {
	id: uuid::Uuid,
	service_id: uuid::Uuid,
	status: StatusDto,
	source: RecordSourceDto,
	description: String,
}

#[derive(Serialize, Deserialize)]
enum RecordSourceDto {
	Prometheus,
	Obm,
	GcpPoll,
	OnPremPoll,
}

#[derive(Serialize, Deserialize)]
enum StatusDto {
	Ok,
	Down,
}
