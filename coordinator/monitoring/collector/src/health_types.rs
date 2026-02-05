use serde::Deserialize;

/// Root response from coordinator's `/health/detailed` endpoint.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DetailedHealth {
    pub status: String,
    pub timestamp: i64,
    pub checks: HealthChecks,
}

#[derive(Debug, Deserialize)]
pub struct HealthChecks {
    pub database: ServiceCheck,
    pub redis: ServiceCheck,
    pub keystore: ServiceCheck,
    pub workers: WorkersCheck,
    pub event_monitor: EventMonitorCheck,
    pub tee_attestation: TeeAttestationCheck,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ServiceCheck {
    pub status: String,
    pub latency_ms: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WorkersCheck {
    pub status: String,
    pub active: i64,
    pub total: i64,
    pub details: Vec<WorkerDetail>,
}

#[derive(Debug, Deserialize)]
pub struct WorkerDetail {
    pub worker_id: String,
    pub worker_name: String,
    pub status: String,
    pub last_heartbeat_secs_ago: i64,
}

#[derive(Debug, Deserialize)]
pub struct EventMonitorCheck {
    pub status: String,
    pub chain_tip_block: Option<i64>,
    pub workers: Vec<EventMonitorWorkerDetail>,
}

#[derive(Debug, Deserialize)]
pub struct EventMonitorWorkerDetail {
    pub worker_id: String,
    pub current_block: Option<i64>,
    pub blocks_behind: Option<i64>,
    pub last_update_secs_ago: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TeeAttestationCheck {
    pub status: String,
    pub workers: Vec<TeeWorkerDetail>,
}

#[derive(Debug, Deserialize)]
pub struct TeeWorkerDetail {
    pub worker_name: String,
    pub last_attestation_secs_ago: Option<i64>,
}
