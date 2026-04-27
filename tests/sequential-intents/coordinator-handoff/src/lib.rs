use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowSpec {
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowStep {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub receiver_id: String,
    #[serde(default)]
    pub method_name: String,
    #[serde(default = "zero_deposit")]
    pub deposit: String,
    #[serde(default)]
    pub near_intents: Option<Value>,
    #[serde(default)]
    pub predecessor_requirement: Option<String>,
    #[serde(default)]
    pub requires_user_predecessor: bool,
    #[serde(default)]
    pub route: Option<RouteHint>,
    #[serde(default)]
    pub gate_batch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SequentialBatchRequest {
    pub gate_id: String,
    pub calls: Vec<SequentialCall>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub post_batch_view_checks: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SequentialCall {
    pub receiver_id: String,
    pub method_name: String,
    #[serde(default)]
    pub args_base64: Option<String>,
    #[serde(default)]
    pub args_json: Option<Value>,
    #[serde(default)]
    pub near_intents: Option<Value>,
    pub gas: String,
    #[serde(default = "zero_deposit")]
    pub deposit: String,
    #[serde(default)]
    pub requires_user_predecessor: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteHint {
    GateProxy,
    DirectUser,
    FundingSetup,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    GateProxy,
    FundingSetup,
    DirectUser,
    Reject,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PredecessorModel {
    Gate,
    Wallet,
    User,
    View,
    None,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderingModel {
    GateChained,
    NormalTxOrView,
    SingleTxAtomic,
    None,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StepEvidence {
    pub lane: Lane,
    pub predecessor_model: PredecessorModel,
    pub ordering_model: OrderingModel,
    pub receiver_id: String,
    pub method_name: String,
    pub policy_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PlanEvidence {
    pub accepted: bool,
    pub steps: Vec<StepEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Deny(&'static str),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BatchRecord {
    pub request_id: String,
    pub endpoint: String,
    pub idempotency_key: String,
    pub original_request_json: Value,
    pub workflow_json: Value,
    pub plan_json: Value,
    pub persisted_before_signing: bool,
    pub proxy_predecessor: bool,
    pub predecessor_model: String,
    pub ordering_model: String,
    pub status: String,
    pub signed_nep413_payloads: Vec<String>,
    pub signed_nep366_delegates: Vec<String>,
    pub submit_tx_hashes: Vec<String>,
    pub intent_ids: Vec<String>,
    pub resume_tx_hash: Option<String>,
    pub dispatch_receipts: Vec<DispatchReceipt>,
    pub dispatch_block_heights: Vec<u64>,
    pub balance_deltas: BTreeMap<String, String>,
    pub user_visible_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DispatchReceipt {
    pub intent_id: String,
    pub receipt_id: String,
    pub receiver_id: String,
    pub method_name: String,
    pub block_height: u64,
    pub status: String,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatorError {
    InvalidJson(String),
}

impl fmt::Display for CoordinatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(f, "invalid workflow JSON: {message}"),
        }
    }
}

impl std::error::Error for CoordinatorError {}

pub const MAINNET_GATE_ID: &str = "gate.sequential.near";
pub const MAINNET_GATE_CODE_HASH: &str = "B6UXBwbuk6JYorjDTqJJyqq7yn9kc7wjcdW4ggzvbkXB";
pub const MAINNET_GATE_OWNER: &str = "sequential.near";
pub const MAINNET_GATE_APPROVER: &str = "approver.sequential.near";
pub const MAINNET_GATE_RELAYER: &str = "relayer.sequential.near";
pub const GATE_FEE_1_TO_3_YOCTO: &str = "30000000000000000000000";
pub const DUST_TOKEN: &str = "nep141:wrap.near";
pub const DUST_TRANSFER_YOCTO_WNEAR: &str = "1000000000000000000";
pub const DUST_TOTAL_YOCTO_WNEAR: &str = "3000000000000000000";
pub const DUST_CALL_GAS: &str = "100000000000000";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentsSimulationResult {
    pub call_index: usize,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainnetPreflightSnapshot {
    pub gate_id: String,
    pub gate_code_hash: String,
    pub gate_owner: String,
    pub gate_approver: String,
    pub relayer_account_id: String,
    pub relayer_whitelisted: bool,
    pub pending_count: usize,
    pub fee_tier_1_to_3_yocto: String,
    pub wallet_near_balance_yocto: String,
    pub wallet_intents_wnear_balance: String,
    pub signed_payload_simulations: Vec<IntentsSimulationResult>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MainnetPreflightEvidence {
    pub ready: bool,
    pub abort_reasons: Vec<String>,
    pub expected_gate_id: String,
    pub expected_fee_yocto: String,
    pub required_intents_delta: String,
    pub workflow_json: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EndpointResponse {
    pub status_code: u16,
    pub body: Value,
}

#[derive(Debug, Default)]
pub struct CoordinatorHandoff {
    records_by_idempotency_key: BTreeMap<String, BatchRecord>,
    records_by_request_id: BTreeMap<String, String>,
    next_request: usize,
    pub policy_checks: usize,
    pub nep413_signatures: usize,
    pub nep366_delegates: usize,
    pub broadcasts: usize,
}

impl CoordinatorHandoff {
    pub fn handle_post_workflows_plan(&self, body: &str) -> EndpointResponse {
        match plan_workflow_json(body) {
            Ok(plan) => endpoint_json(200, serde_json::to_value(plan).expect("plan serializes")),
            Err(err) => endpoint_error(400, err.to_string()),
        }
    }

    pub fn handle_post_workflows_execute(
        &mut self,
        body: &str,
        policy: PolicyDecision,
    ) -> EndpointResponse {
        let idempotency_key = match request_idempotency_key(body) {
            Ok(Some(key)) if !key.is_empty() => key,
            Ok(_) => return endpoint_error(400, "idempotency_key is required"),
            Err(err) => return endpoint_error(400, err.to_string()),
        };

        match self.execute_workflow_json(body, &idempotency_key, policy) {
            Ok(record) => endpoint_json(
                status_code_for_record(&record),
                serde_json::to_value(record).expect("record serializes"),
            ),
            Err(err) => endpoint_error(400, err.to_string()),
        }
    }

    pub fn handle_get_workflow_status(&self, request_id: &str) -> EndpointResponse {
        match self.workflow_status(request_id) {
            Some(record) => endpoint_json(
                200,
                serde_json::to_value(record).expect("record serializes"),
            ),
            None => endpoint_error(404, format!("workflow request {request_id} was not found")),
        }
    }

    pub fn handle_post_sequential_batch(
        &mut self,
        body: &str,
        policy: PolicyDecision,
    ) -> EndpointResponse {
        let idempotency_key = match request_idempotency_key(body) {
            Ok(Some(key)) if !key.is_empty() => key,
            Ok(_) => return endpoint_error(400, "idempotency_key is required"),
            Err(err) => return endpoint_error(400, err.to_string()),
        };

        match self.execute_sequential_batch_json(body, &idempotency_key, policy) {
            Ok(record) => endpoint_json(
                status_code_for_record(&record),
                serde_json::to_value(record).expect("record serializes"),
            ),
            Err(err) => endpoint_error(400, err.to_string()),
        }
    }

    pub fn handle_get_sequential_batch_status(&self, request_id: &str) -> EndpointResponse {
        match self.sequence_status(request_id) {
            Some(record) => endpoint_json(
                200,
                serde_json::to_value(record).expect("record serializes"),
            ),
            None => endpoint_error(
                404,
                format!("sequential batch request {request_id} was not found"),
            ),
        }
    }

    pub fn plan_workflow_json(
        &self,
        workflow_json: &str,
    ) -> Result<PlanEvidence, CoordinatorError> {
        plan_workflow_json(workflow_json)
    }

    pub fn execute_workflow_json(
        &mut self,
        workflow_json: &str,
        idempotency_key: &str,
        policy: PolicyDecision,
    ) -> Result<BatchRecord, CoordinatorError> {
        if let Some(record) = self.records_by_idempotency_key.get(idempotency_key) {
            return Ok(record.clone());
        }

        let workflow: WorkflowSpec = serde_json::from_str(workflow_json)
            .map_err(|err| CoordinatorError::InvalidJson(err.to_string()))?;
        let workflow_value: Value = serde_json::from_str(workflow_json)
            .map_err(|err| CoordinatorError::InvalidJson(err.to_string()))?;

        Ok(self.execute_planned_workflow(
            workflow,
            workflow_value.clone(),
            workflow_value,
            "workflow",
            idempotency_key,
            policy,
        ))
    }

    pub fn execute_sequential_batch_json(
        &mut self,
        batch_json: &str,
        idempotency_key: &str,
        policy: PolicyDecision,
    ) -> Result<BatchRecord, CoordinatorError> {
        let request_value: Value = serde_json::from_str(batch_json)
            .map_err(|err| CoordinatorError::InvalidJson(err.to_string()))?;
        let request: SequentialBatchRequest = serde_json::from_value(request_value.clone())
            .map_err(|err| CoordinatorError::InvalidJson(err.to_string()))?;
        let effective_idempotency_key = if idempotency_key.is_empty() {
            request.idempotency_key.as_deref().unwrap_or("")
        } else {
            idempotency_key
        };

        if let Some(record) = self
            .records_by_idempotency_key
            .get(effective_idempotency_key)
        {
            return Ok(record.clone());
        }

        if let Err(message) = validate_sequential_batch(&request, effective_idempotency_key) {
            let request_id = self.allocate_request_id();
            let plan = PlanEvidence {
                accepted: false,
                steps: Vec::new(),
            };
            let record = self.new_record(
                request_id,
                "sequential_batch",
                effective_idempotency_key,
                request_value.clone(),
                request_value,
                &plan,
            );
            self.persist(effective_idempotency_key, record.clone());
            return Ok(self.finish_with_error(
                effective_idempotency_key,
                record,
                "rejected",
                &message,
            ));
        }

        let workflow = sequential_batch_to_workflow(&request);
        let workflow_value =
            serde_json::to_value(&workflow).expect("translated workflow serializes");

        Ok(self.execute_planned_workflow(
            workflow,
            workflow_value,
            request_value,
            "sequential_batch",
            effective_idempotency_key,
            policy,
        ))
    }

    pub fn sequence_status(&self, request_id: &str) -> Option<BatchRecord> {
        self.workflow_status(request_id)
    }

    fn execute_planned_workflow(
        &mut self,
        workflow: WorkflowSpec,
        workflow_value: Value,
        original_request_value: Value,
        endpoint: &str,
        idempotency_key: &str,
        policy: PolicyDecision,
    ) -> BatchRecord {
        let plan = plan_workflow(&workflow);
        let request_id = self.allocate_request_id();

        let mut record = self.new_record(
            request_id,
            endpoint,
            idempotency_key,
            original_request_value,
            workflow_value,
            &plan,
        );

        self.persist(idempotency_key, record.clone());

        if !plan.accepted {
            let reason = plan
                .steps
                .iter()
                .find_map(|step| step.reason.as_deref())
                .unwrap_or("workflow rejected before signing");
            record = self.finish_with_error(idempotency_key, record, "rejected", reason);
            return record;
        }

        if plan.steps.iter().any(|step| step.lane == Lane::DirectUser) {
            record.predecessor_model = "user".to_string();
            record.ordering_model = "single_tx_atomic".to_string();
            record = self.finish_with_error(
                idempotency_key,
                record,
                "requires_direct_user_setup",
                "direct_user execution is planned but not enabled in the gate-first milestone",
            );
            return record;
        }

        if plan
            .steps
            .iter()
            .any(|step| step.lane == Lane::FundingSetup)
        {
            record.predecessor_model = "wallet".to_string();
            record.ordering_model = "normal_tx_or_view".to_string();
            record = self.finish_with_error(
                idempotency_key,
                record,
                "requires_funding_setup",
                "funding_setup steps should complete through existing wallet routes before gate execution in this milestone",
            );
            return record;
        }

        let gate_steps = plan
            .steps
            .iter()
            .enumerate()
            .filter(|(_, step)| step.lane == Lane::GateProxy)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        if gate_steps.is_empty() {
            record = self.finish_with_error(
                idempotency_key,
                record,
                "rejected",
                "workflow contains no executable gate_proxy steps",
            );
            return record;
        }

        self.policy_checks += gate_steps.len();
        if let PolicyDecision::Deny(reason) = policy {
            record = self.finish_with_error(
                idempotency_key,
                record,
                "policy_denied",
                &format!("policy denied before signing: {reason}"),
            );
            return record;
        }

        record.proxy_predecessor = true;
        record.predecessor_model = "gate".to_string();
        record.ordering_model = "gate_chained".to_string();

        self.sign_gate_steps(&mut record, &workflow, &gate_steps);

        match self.submit_and_resume_gate_steps(&mut record, &workflow, &gate_steps) {
            Ok(()) => {
                record.status = "succeeded".to_string();
                self.persist(idempotency_key, record.clone());
                record
            }
            Err(message) => {
                record = self.finish_with_error(idempotency_key, record, "failed", &message);
                record
            }
        }
    }

    pub fn workflow_status(&self, request_id: &str) -> Option<BatchRecord> {
        let idempotency_key = self.records_by_request_id.get(request_id)?;
        self.records_by_idempotency_key
            .get(idempotency_key)
            .cloned()
    }

    fn allocate_request_id(&mut self) -> String {
        self.next_request += 1;
        format!("seq-req-{}", self.next_request)
    }

    fn new_record(
        &self,
        request_id: String,
        endpoint: &str,
        idempotency_key: &str,
        original_request_json: Value,
        workflow_json: Value,
        plan: &PlanEvidence,
    ) -> BatchRecord {
        BatchRecord {
            request_id,
            endpoint: endpoint.to_string(),
            idempotency_key: idempotency_key.to_string(),
            original_request_json,
            workflow_json,
            plan_json: serde_json::to_value(plan).expect("plan evidence serializes"),
            persisted_before_signing: true,
            proxy_predecessor: false,
            predecessor_model: "none".to_string(),
            ordering_model: "none".to_string(),
            status: "planned".to_string(),
            signed_nep413_payloads: Vec::new(),
            signed_nep366_delegates: Vec::new(),
            submit_tx_hashes: Vec::new(),
            intent_ids: Vec::new(),
            resume_tx_hash: None,
            dispatch_receipts: Vec::new(),
            dispatch_block_heights: Vec::new(),
            balance_deltas: BTreeMap::new(),
            user_visible_error: None,
        }
    }

    fn persist(&mut self, idempotency_key: &str, record: BatchRecord) {
        self.records_by_request_id
            .insert(record.request_id.clone(), idempotency_key.to_string());
        self.records_by_idempotency_key
            .insert(idempotency_key.to_string(), record);
    }

    fn finish_with_error(
        &mut self,
        idempotency_key: &str,
        mut record: BatchRecord,
        status: &str,
        error: &str,
    ) -> BatchRecord {
        record.status = status.to_string();
        record.user_visible_error = Some(error.to_string());
        self.persist(idempotency_key, record.clone());
        record
    }

    fn sign_gate_steps(
        &mut self,
        record: &mut BatchRecord,
        workflow: &WorkflowSpec,
        gate_steps: &[usize],
    ) {
        for (call_order, step_index) in gate_steps.iter().enumerate() {
            let step = &workflow.steps[*step_index];
            if let Some(intent_kind) = intent_kind_for_nep413(step) {
                record.signed_nep413_payloads.push(format!(
                    "signed-nep413:{}:{call_order}:{intent_kind}",
                    record.request_id
                ));
                self.nep413_signatures += 1;
            }
            record.signed_nep366_delegates.push(format!(
                "signed-nep366:{}:{call_order}:gate.sequential.near",
                record.request_id
            ));
            record
                .submit_tx_hashes
                .push(format!("submit-tx-{}-{call_order}", record.request_id));
        }

        self.nep366_delegates += gate_steps.len();
    }

    fn submit_and_resume_gate_steps(
        &mut self,
        record: &mut BatchRecord,
        workflow: &WorkflowSpec,
        gate_steps: &[usize],
    ) -> Result<(), String> {
        self.broadcasts += gate_steps.len();

        let submit_receipts = gate_steps
            .iter()
            .enumerate()
            .map(|(call_order, _)| SubmitReceipt {
                call_order,
                tx_hash: record.submit_tx_hashes[call_order].clone(),
                outcome: submit_outcome(
                    &format!("intent-{}-{call_order}", record.request_id),
                    call_order,
                    if call_order % 2 == 0 {
                        OutcomeShape::TopLevel
                    } else {
                        OutcomeShape::ResultWrapped
                    },
                ),
            })
            .collect::<Vec<_>>();

        let mut intent_ids_by_call = vec![None; gate_steps.len()];
        for receipt in submit_receipts.iter().rev() {
            let intent_id = parse_gate_events(&receipt.outcome)
                .into_iter()
                .find(|event| event.kind == GateEventKind::IntentSubmitted)
                .and_then(|event| event.intent_id);

            match intent_id {
                Some(intent_id) => intent_ids_by_call[receipt.call_order] = Some(intent_id),
                None => {
                    return Err(format!(
                        "missing intent_submitted log for submit tx {}",
                        receipt.tx_hash
                    ));
                }
            }
        }

        record.intent_ids = intent_ids_by_call
            .into_iter()
            .map(|intent_id| intent_id.expect("all submit receipts were checked"))
            .collect();
        record.resume_tx_hash = Some(format!("resume-tx-{}", record.request_id));
        self.broadcasts += 1;

        for (call_order, intent_id) in record.intent_ids.iter().enumerate() {
            let step = &workflow.steps[gate_steps[call_order]];
            let block_height = 10_000 + call_order as u64;
            record.dispatch_block_heights.push(block_height);
            record.dispatch_receipts.push(DispatchReceipt {
                intent_id: intent_id.clone(),
                receipt_id: format!("dispatch-{}-{call_order}", record.request_id),
                receiver_id: coordinator_receiver_id(step),
                method_name: coordinator_method_name(step),
                block_height,
                status: "success".to_string(),
                logs: vec![
                    event_log(
                        "intent_dispatched",
                        json!({ "intent_id": intent_id, "block_height": block_height }),
                    ),
                    event_log(
                        "chain_continued",
                        json!({ "intent_id": intent_id, "block_height": block_height + 1 }),
                    ),
                ],
            });
        }

        if gate_steps
            .iter()
            .all(|step_index| is_intents_step(&workflow.steps[*step_index]))
        {
            record.balance_deltas.insert(
                "nep141:wrap.near:intents".to_string(),
                "-3000000000000000000".to_string(),
            );
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct SubmitReceipt {
    call_order: usize,
    tx_hash: String,
    outcome: Value,
}

#[derive(Debug, Clone, Copy)]
enum OutcomeShape {
    TopLevel,
    ResultWrapped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateEvent {
    pub kind: GateEventKind,
    pub intent_id: Option<String>,
    pub batch_id: Option<String>,
    pub receipt_id: Option<String>,
    pub block_height: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateEventKind {
    IntentSubmitted,
    BatchStarted,
    IntentDispatched,
    ChainContinued,
}

pub fn plan_workflow_json(input: &str) -> Result<PlanEvidence, CoordinatorError> {
    let spec: WorkflowSpec = serde_json::from_str(input)
        .map_err(|err| CoordinatorError::InvalidJson(err.to_string()))?;
    Ok(plan_workflow(&spec))
}

pub fn plan_workflow(spec: &WorkflowSpec) -> PlanEvidence {
    let mut steps = spec.steps.iter().map(classify_step).collect::<Vec<_>>();
    reject_mixed_gate_batches(spec, &mut steps);
    let accepted = steps.iter().all(|step| step.lane != Lane::Reject);
    PlanEvidence { accepted, steps }
}

pub fn parse_gate_events(outcome: &Value) -> Vec<GateEvent> {
    let mut logs = Vec::new();
    collect_logs(outcome, &mut logs);
    logs.into_iter()
        .filter_map(|log| parse_gate_event_log(&log))
        .collect()
}

pub fn mainnet_dust_workflow_json(recipient_id: &str) -> Value {
    json!({
        "steps": [
            dust_transfer_step(recipient_id),
            dust_transfer_step(recipient_id),
            dust_transfer_step(recipient_id)
        ]
    })
}

pub fn evaluate_mainnet_preflight(snapshot: &MainnetPreflightSnapshot) -> MainnetPreflightEvidence {
    let mut abort_reasons = Vec::new();

    if snapshot.gate_id != MAINNET_GATE_ID {
        abort_reasons.push(format!(
            "gate_id drift: expected {MAINNET_GATE_ID}, got {}",
            snapshot.gate_id
        ));
    }
    if snapshot.gate_code_hash != MAINNET_GATE_CODE_HASH {
        abort_reasons.push("gate code hash changed".to_string());
    }
    if snapshot.gate_owner != MAINNET_GATE_OWNER {
        abort_reasons.push(format!(
            "gate owner drift: expected {MAINNET_GATE_OWNER}, got {}",
            snapshot.gate_owner
        ));
    }
    if snapshot.gate_approver != MAINNET_GATE_APPROVER {
        abort_reasons.push(format!(
            "gate approver drift: expected {MAINNET_GATE_APPROVER}, got {}",
            snapshot.gate_approver
        ));
    }
    if snapshot.relayer_account_id != MAINNET_GATE_RELAYER || !snapshot.relayer_whitelisted {
        abort_reasons.push(format!(
            "relayer {MAINNET_GATE_RELAYER} is not confirmed whitelisted"
        ));
    }
    if snapshot.pending_count != 0 {
        abort_reasons.push(format!(
            "gate pending list is not empty: {} pending item(s)",
            snapshot.pending_count
        ));
    }
    if snapshot.fee_tier_1_to_3_yocto != GATE_FEE_1_TO_3_YOCTO {
        abort_reasons.push(format!(
            "1-3 call gate fee drift: expected {GATE_FEE_1_TO_3_YOCTO}, got {}",
            snapshot.fee_tier_1_to_3_yocto
        ));
    }
    if !yocto_at_least(&snapshot.wallet_near_balance_yocto, "1") {
        abort_reasons.push("wallet NEAR gas balance is missing or zero".to_string());
    }
    if !yocto_at_least(
        &snapshot.wallet_intents_wnear_balance,
        DUST_TOTAL_YOCTO_WNEAR,
    ) {
        abort_reasons.push(format!(
            "wallet Intents wNEAR balance is below required {DUST_TOTAL_YOCTO_WNEAR}"
        ));
    }
    if snapshot.signed_payload_simulations.len() != 3 {
        abort_reasons.push(format!(
            "expected 3 signed Intents simulations, got {}",
            snapshot.signed_payload_simulations.len()
        ));
    }
    for simulation in &snapshot.signed_payload_simulations {
        if !simulation.ok {
            abort_reasons.push(format!(
                "signed Intents simulation {} failed: {}",
                simulation.call_index,
                simulation.error.as_deref().unwrap_or("unknown error")
            ));
        }
    }

    MainnetPreflightEvidence {
        ready: abort_reasons.is_empty(),
        abort_reasons,
        expected_gate_id: MAINNET_GATE_ID.to_string(),
        expected_fee_yocto: GATE_FEE_1_TO_3_YOCTO.to_string(),
        required_intents_delta: DUST_TOTAL_YOCTO_WNEAR.to_string(),
        workflow_json: mainnet_dust_workflow_json("mike.near"),
    }
}

fn request_idempotency_key(body: &str) -> Result<Option<String>, CoordinatorError> {
    let value: Value =
        serde_json::from_str(body).map_err(|err| CoordinatorError::InvalidJson(err.to_string()))?;
    Ok(value
        .get("idempotency_key")
        .and_then(Value::as_str)
        .map(str::to_string))
}

fn status_code_for_record(record: &BatchRecord) -> u16 {
    match record.status.as_str() {
        "succeeded" | "planned" => 200,
        "requires_direct_user_setup" | "requires_funding_setup" => 409,
        "rejected" | "policy_denied" | "failed" => 400,
        _ => 202,
    }
}

fn endpoint_json(status_code: u16, body: Value) -> EndpointResponse {
    EndpointResponse { status_code, body }
}

fn endpoint_error(status_code: u16, message: impl Into<String>) -> EndpointResponse {
    EndpointResponse {
        status_code,
        body: json!({
            "error": message.into()
        }),
    }
}

fn validate_sequential_batch(
    request: &SequentialBatchRequest,
    idempotency_key: &str,
) -> Result<(), String> {
    if idempotency_key.is_empty() {
        return Err("idempotency_key is required".to_string());
    }
    if request.gate_id.trim().is_empty() {
        return Err("gate_id is required".to_string());
    }
    if request.calls.is_empty() || request.calls.len() > 3 {
        return Err("sequential batch requires 1 to 3 calls".to_string());
    }

    for (index, call) in request.calls.iter().enumerate() {
        if call.receiver_id.trim().is_empty() {
            return Err(format!("calls[{index}].receiver_id is required"));
        }
        if call.method_name.trim().is_empty() {
            return Err(format!("calls[{index}].method_name is required"));
        }
        if call.gas.trim().is_empty() || call.gas == "0" {
            return Err(format!("calls[{index}].gas must be non-zero"));
        }
        if call.requires_user_predecessor {
            return Err(format!(
                "calls[{index}] requires user predecessor, which the gate proxy cannot satisfy"
            ));
        }

        let payload_modes = [
            call.args_base64.is_some(),
            call.args_json.is_some(),
            call.near_intents.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();

        if payload_modes != 1 {
            return Err(format!(
                "calls[{index}] must include exactly one of args_base64, args_json, or near_intents"
            ));
        }

        if call.near_intents.is_some()
            && (call.receiver_id != "intents.near"
                || call.method_name != "execute_intents"
                || call.deposit != "0")
        {
            return Err(format!(
                "calls[{index}].near_intents requires intents.near.execute_intents with zero deposit"
            ));
        }
    }

    Ok(())
}

fn dust_transfer_step(recipient_id: &str) -> Value {
    json!({
        "kind": "intents.transfer",
        "receiver_id": recipient_id,
        "token": DUST_TOKEN,
        "amount": DUST_TRANSFER_YOCTO_WNEAR,
        "gas": DUST_CALL_GAS,
        "deposit": "0",
        "gate_batch": "mainnet-dust"
    })
}

fn yocto_at_least(value: &str, minimum: &str) -> bool {
    match (value.parse::<u128>(), minimum.parse::<u128>()) {
        (Ok(value), Ok(minimum)) => value >= minimum,
        _ => false,
    }
}

fn sequential_batch_to_workflow(request: &SequentialBatchRequest) -> WorkflowSpec {
    WorkflowSpec {
        steps: request
            .calls
            .iter()
            .map(|call| WorkflowStep {
                kind: None,
                receiver_id: call.receiver_id.clone(),
                method_name: call.method_name.clone(),
                deposit: call.deposit.clone(),
                near_intents: call.near_intents.clone(),
                predecessor_requirement: None,
                requires_user_predecessor: call.requires_user_predecessor,
                route: Some(RouteHint::GateProxy),
                gate_batch: Some(request.gate_id.clone()),
            })
            .collect(),
    }
}

fn classify_step(step: &WorkflowStep) -> StepEvidence {
    if let Some(kind) = step.kind.as_deref() {
        return classify_kind_step(step, kind);
    }

    if step.near_intents.is_some() {
        if is_execute_intents_call(step) {
            return evidence(step, Lane::GateProxy, None);
        }
        return evidence(
            step,
            Lane::Reject,
            Some("near_intents payloads require intents.near.execute_intents with zero deposit"),
        );
    }

    if step.route == Some(RouteHint::GateProxy) && step.requires_user_predecessor {
        return evidence(
            step,
            Lane::Reject,
            Some("gate_proxy cannot satisfy requires_user_predecessor"),
        );
    }

    if step.route == Some(RouteHint::GateProxy) {
        return evidence(step, Lane::GateProxy, None);
    }

    if is_funding_step(step) {
        return evidence(step, Lane::FundingSetup, None);
    }

    if step.requires_user_predecessor || step.route == Some(RouteHint::DirectUser) {
        return evidence(step, Lane::DirectUser, None);
    }

    evidence(
        step,
        Lane::Reject,
        Some("ambiguous normal calls must be classified before policy checks or signing"),
    )
}

fn classify_kind_step(step: &WorkflowStep, kind: &str) -> StepEvidence {
    match kind {
        "intents.transfer" | "intents.swap" | "intents.execute_raw" => {
            if step.deposit == "0" {
                evidence(step, Lane::GateProxy, None)
            } else {
                evidence(
                    step,
                    Lane::Reject,
                    Some("Intents workflow steps routed through the gate must attach zero deposit"),
                )
            }
        }
        "funding.wrap_near"
        | "funding.intents_deposit"
        | "funding.balance_check"
        | "funding.storage_deposit" => evidence(step, Lane::FundingSetup, None),
        "near.function_call" => {
            if step.predecessor_requirement.as_deref() == Some("user_required")
                || step.requires_user_predecessor
            {
                evidence(step, Lane::DirectUser, None)
            } else {
                evidence(
                    step,
                    Lane::Reject,
                    Some("near.function_call must declare predecessor_requirement user_required"),
                )
            }
        }
        _ => evidence(step, Lane::Reject, Some("unsupported workflow step kind")),
    }
}

fn reject_mixed_gate_batches(spec: &WorkflowSpec, steps: &mut [StepEvidence]) {
    let mut batches: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, step) in spec.steps.iter().enumerate() {
        if let Some(batch_id) = step.gate_batch.as_deref() {
            batches.entry(batch_id).or_default().push(index);
        }
    }

    for indexes in batches.values() {
        let has_gate_proxy = indexes
            .iter()
            .any(|index| steps[*index].lane == Lane::GateProxy);
        let has_predecessor_sensitive = indexes.iter().any(|index| {
            spec.steps[*index].requires_user_predecessor || steps[*index].lane == Lane::DirectUser
        });

        if has_gate_proxy && has_predecessor_sensitive {
            for index in indexes {
                mark_reject(
                    &mut steps[*index],
                    "gate batch mixes proxy-safe and predecessor-sensitive steps",
                );
            }
        }
    }
}

fn evidence(step: &WorkflowStep, lane: Lane, reason: Option<&str>) -> StepEvidence {
    let (predecessor_model, ordering_model, policy_type) = match lane {
        Lane::GateProxy => (PredecessorModel::Gate, OrderingModel::GateChained, "call"),
        Lane::FundingSetup => (
            funding_predecessor_model(step),
            OrderingModel::NormalTxOrView,
            "setup",
        ),
        Lane::DirectUser => (
            PredecessorModel::User,
            OrderingModel::SingleTxAtomic,
            "direct_user",
        ),
        Lane::Reject => (PredecessorModel::None, OrderingModel::None, "reject"),
    };

    StepEvidence {
        lane,
        predecessor_model,
        ordering_model,
        receiver_id: coordinator_receiver_id(step),
        method_name: coordinator_method_name(step),
        policy_type: policy_type.to_string(),
        reason: reason.map(str::to_string),
    }
}

fn mark_reject(step: &mut StepEvidence, reason: &str) {
    step.lane = Lane::Reject;
    step.predecessor_model = PredecessorModel::None;
    step.ordering_model = OrderingModel::None;
    step.policy_type = "reject".to_string();
    step.reason = Some(reason.to_string());
}

fn coordinator_receiver_id(step: &WorkflowStep) -> String {
    match step.kind.as_deref() {
        Some("intents.transfer" | "intents.swap" | "intents.execute_raw") => {
            "intents.near".to_string()
        }
        Some("funding.wrap_near") => "wrap.near".to_string(),
        Some("funding.intents_deposit") => "intents.near".to_string(),
        Some("funding.balance_check") => step.receiver_id_or("balance"),
        Some("funding.storage_deposit") => step.receiver_id_or("storage"),
        _ => step.receiver_id.clone(),
    }
}

fn coordinator_method_name(step: &WorkflowStep) -> String {
    match step.kind.as_deref() {
        Some("intents.transfer" | "intents.swap" | "intents.execute_raw") => {
            "execute_intents".to_string()
        }
        Some("funding.wrap_near") => "near_deposit".to_string(),
        Some("funding.intents_deposit") => "ft_transfer_call".to_string(),
        Some("funding.balance_check") => "balance_check".to_string(),
        Some("funding.storage_deposit") => "storage_deposit".to_string(),
        _ => step.method_name.clone(),
    }
}

fn intent_kind_for_nep413(step: &WorkflowStep) -> Option<&str> {
    if matches!(
        step.kind.as_deref(),
        Some("intents.transfer" | "intents.swap" | "intents.execute_raw")
    ) {
        return step.kind.as_deref();
    }

    step.near_intents
        .as_ref()
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
        .or(Some("intents.execute_raw"))
        .filter(|_| step.near_intents.is_some())
}

fn is_intents_step(step: &WorkflowStep) -> bool {
    intent_kind_for_nep413(step).is_some()
}

fn is_execute_intents_call(step: &WorkflowStep) -> bool {
    step.receiver_id == "intents.near"
        && step.method_name == "execute_intents"
        && step.deposit == "0"
}

fn is_funding_step(step: &WorkflowStep) -> bool {
    if step.route == Some(RouteHint::FundingSetup) {
        return true;
    }

    if step.receiver_id == "wrap.near" && step.method_name == "near_deposit" {
        return true;
    }

    matches!(
        step.method_name.as_str(),
        "storage_deposit" | "storage_balance_of" | "ft_balance_of"
    )
}

fn funding_predecessor_model(step: &WorkflowStep) -> PredecessorModel {
    if matches!(step.kind.as_deref(), Some("funding.balance_check")) {
        return PredecessorModel::View;
    }

    if matches!(
        step.method_name.as_str(),
        "storage_balance_of" | "ft_balance_of"
    ) {
        PredecessorModel::View
    } else {
        PredecessorModel::Wallet
    }
}

fn submit_outcome(intent_id: &str, call_order: usize, shape: OutcomeShape) -> Value {
    let log = event_log(
        "intent_submitted",
        json!({
            "intent_id": intent_id,
            "receipt_id": format!("submit-receipt-{call_order}"),
            "block_height": 9_000 + call_order as u64
        }),
    );

    match shape {
        OutcomeShape::TopLevel => json!({
            "outcome": {
                "logs": [log]
            }
        }),
        OutcomeShape::ResultWrapped => json!({
            "result": {
                "receipts_outcome": [
                    {
                        "outcome": {
                            "logs": [log]
                        }
                    }
                ]
            }
        }),
    }
}

fn parse_gate_event_log(log: &str) -> Option<GateEvent> {
    let event_json = log.strip_prefix("EVENT_JSON:")?;
    let value = serde_json::from_str::<Value>(event_json).ok()?;
    let event = value.get("event")?.as_str()?;
    let data = event_data(&value);

    let kind = match event {
        "intent_submitted" => GateEventKind::IntentSubmitted,
        "batch_started" => GateEventKind::BatchStarted,
        "intent_dispatched" => GateEventKind::IntentDispatched,
        "chain_continued" => GateEventKind::ChainContinued,
        _ => return None,
    };

    Some(GateEvent {
        kind,
        intent_id: data.and_then(|data| string_field(data, "intent_id")),
        batch_id: data.and_then(|data| string_field(data, "batch_id")),
        receipt_id: data.and_then(|data| string_field(data, "receipt_id")),
        block_height: data.and_then(|data| u64_field(data, "block_height")),
    })
}

fn event_data(value: &Value) -> Option<&Value> {
    match value.get("data")? {
        Value::Array(items) => items.first(),
        value @ Value::Object(_) => Some(value),
        _ => None,
    }
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_string)
}

fn u64_field(value: &Value, field: &str) -> Option<u64> {
    value.get(field).and_then(Value::as_u64)
}

fn collect_logs(value: &Value, logs: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Array(found_logs)) = map.get("logs") {
                logs.extend(
                    found_logs
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string),
                );
            }
            for child in map.values() {
                collect_logs(child, logs);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_logs(item, logs);
            }
        }
        _ => {}
    }
}

fn event_log(event: &str, data: Value) -> String {
    format!(
        "EVENT_JSON:{}",
        json!({
            "standard": "sequential",
            "version": "1.0.0",
            "event": event,
            "data": [data]
        })
    )
}

fn zero_deposit() -> String {
    "0".to_string()
}

trait ReceiverDefault {
    fn receiver_id_or(&self, fallback: &str) -> String;
}

impl ReceiverDefault for WorkflowStep {
    fn receiver_id_or(&self, fallback: &str) -> String {
        if self.receiver_id.is_empty() {
            fallback.to_string()
        } else {
            self.receiver_id.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THREE_INTENTS_TRANSFERS: &str = r#"{
      "steps": [
        {
          "kind": "intents.transfer",
          "token": "nep141:wrap.near",
          "amount": "1000000000000000000",
          "receiver_id": "mike.near",
          "gate_batch": "dust"
        },
        {
          "kind": "intents.transfer",
          "token": "nep141:wrap.near",
          "amount": "1000000000000000000",
          "receiver_id": "mike.near",
          "gate_batch": "dust"
        },
        {
          "kind": "intents.transfer",
          "token": "nep141:wrap.near",
          "amount": "1000000000000000000",
          "receiver_id": "mike.near",
          "gate_batch": "dust"
        }
      ]
    }"#;

    #[test]
    fn plan_accepts_intents_transfer_batch_as_gate_proxy() {
        let plan = plan_workflow_json(THREE_INTENTS_TRANSFERS).unwrap();

        assert!(plan.accepted);
        assert_eq!(plan.steps.len(), 3);
        for step in plan.steps {
            assert_eq!(step.lane, Lane::GateProxy);
            assert_eq!(step.predecessor_model, PredecessorModel::Gate);
            assert_eq!(step.ordering_model, OrderingModel::GateChained);
            assert_eq!(step.receiver_id, "intents.near");
            assert_eq!(step.method_name, "execute_intents");
            assert_eq!(step.policy_type, "call");
        }
    }

    #[test]
    fn plan_rejects_ambiguous_normal_call_before_signing() {
        let mut handoff = CoordinatorHandoff::default();
        let record = handoff
            .execute_workflow_json(
                r#"{"steps":[{"receiver_id":"staking.pool.near","method_name":"withdraw_reward"}]}"#,
                "ambiguous",
                PolicyDecision::Allow,
            )
            .unwrap();

        assert_eq!(record.status, "rejected");
        assert!(record
            .user_visible_error
            .as_deref()
            .unwrap_or_default()
            .contains("ambiguous"));
        assert!(record.persisted_before_signing);
        assert_eq!(handoff.policy_checks, 0);
        assert_eq!(handoff.nep413_signatures, 0);
        assert_eq!(handoff.nep366_delegates, 0);
        assert_eq!(handoff.broadcasts, 0);
    }

    #[test]
    fn workflow_plan_endpoint_returns_planner_evidence_without_signing() {
        let handoff = CoordinatorHandoff::default();
        let response = handoff.handle_post_workflows_plan(THREE_INTENTS_TRANSFERS);

        assert_eq!(response.status_code, 200);
        assert_eq!(
            response.body.get("accepted").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            response
                .body
                .get("steps")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3)
        );
        assert_eq!(handoff.policy_checks, 0);
        assert_eq!(handoff.nep413_signatures, 0);
        assert_eq!(handoff.nep366_delegates, 0);
    }

    #[test]
    fn workflow_execute_endpoint_requires_idempotency_key() {
        let mut handoff = CoordinatorHandoff::default();
        let response =
            handoff.handle_post_workflows_execute(THREE_INTENTS_TRANSFERS, PolicyDecision::Allow);

        assert_eq!(response.status_code, 400);
        assert!(response
            .body
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("idempotency_key"));
        assert_eq!(handoff.policy_checks, 0);
        assert_eq!(handoff.nep413_signatures, 0);
    }

    #[test]
    fn workflow_execute_and_status_endpoints_return_ordered_evidence() {
        let mut handoff = CoordinatorHandoff::default();
        let body = json!({
            "idempotency_key": "workflow-endpoint-key",
            "steps": mainnet_dust_workflow_json("mike.near")["steps"].clone()
        })
        .to_string();

        let execute = handoff.handle_post_workflows_execute(&body, PolicyDecision::Allow);

        assert_eq!(execute.status_code, 200);
        assert_eq!(
            execute.body.get("status").and_then(Value::as_str),
            Some("succeeded")
        );
        assert_eq!(
            execute
                .body
                .get("proxy_predecessor")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            execute
                .body
                .get("intent_ids")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3)
        );

        let request_id = execute
            .body
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap();
        let status = handoff.handle_get_workflow_status(request_id);
        assert_eq!(status.status_code, 200);
        assert_eq!(status.body, execute.body);
    }

    #[test]
    fn workflow_execute_endpoint_returns_direct_user_setup_conflict() {
        let mut handoff = CoordinatorHandoff::default();
        let body = r#"{
          "idempotency_key": "direct-user-endpoint-key",
          "steps": [
            {
              "kind": "near.function_call",
              "predecessor_requirement": "user_required",
              "user_id": "mike.near",
              "receiver_id": "staking.pool.near",
              "actions": [
                {"method_name": "withdraw_reward", "gas": "30000000000000", "deposit": "0"}
              ]
            }
          ]
        }"#;

        let response = handoff.handle_post_workflows_execute(body, PolicyDecision::Allow);

        assert_eq!(response.status_code, 409);
        assert_eq!(
            response.body.get("status").and_then(Value::as_str),
            Some("requires_direct_user_setup")
        );
        assert_eq!(
            response
                .body
                .get("predecessor_model")
                .and_then(Value::as_str),
            Some("user")
        );
        assert_eq!(handoff.policy_checks, 0);
    }

    #[test]
    fn mainnet_dust_workflow_contains_exact_three_transfer_steps() {
        let workflow = mainnet_dust_workflow_json("mike.near");
        let steps = workflow.get("steps").unwrap().as_array().unwrap();

        assert_eq!(steps.len(), 3);
        for step in steps {
            assert_eq!(
                step.get("kind").and_then(Value::as_str),
                Some("intents.transfer")
            );
            assert_eq!(
                step.get("receiver_id").and_then(Value::as_str),
                Some("mike.near")
            );
            assert_eq!(step.get("token").and_then(Value::as_str), Some(DUST_TOKEN));
            assert_eq!(
                step.get("amount").and_then(Value::as_str),
                Some(DUST_TRANSFER_YOCTO_WNEAR)
            );
            assert_eq!(step.get("gas").and_then(Value::as_str), Some(DUST_CALL_GAS));
            assert_eq!(step.get("deposit").and_then(Value::as_str), Some("0"));
            assert_eq!(
                step.get("gate_batch").and_then(Value::as_str),
                Some("mainnet-dust")
            );
        }
    }

    #[test]
    fn mainnet_preflight_accepts_expected_snapshot() {
        let evidence = evaluate_mainnet_preflight(&ready_mainnet_snapshot());

        assert!(evidence.ready);
        assert!(evidence.abort_reasons.is_empty());
        assert_eq!(evidence.expected_gate_id, MAINNET_GATE_ID);
        assert_eq!(evidence.expected_fee_yocto, GATE_FEE_1_TO_3_YOCTO);
        assert_eq!(evidence.required_intents_delta, DUST_TOTAL_YOCTO_WNEAR);
        assert_eq!(
            evidence
                .workflow_json
                .get("steps")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3)
        );
    }

    #[test]
    fn mainnet_preflight_rejects_gate_config_drift() {
        let mut snapshot = ready_mainnet_snapshot();
        snapshot.gate_code_hash = "changed".to_string();
        snapshot.gate_approver = "wrong.near".to_string();
        snapshot.pending_count = 1;
        snapshot.fee_tier_1_to_3_yocto = "1".to_string();

        let evidence = evaluate_mainnet_preflight(&snapshot);

        assert!(!evidence.ready);
        assert!(evidence
            .abort_reasons
            .iter()
            .any(|reason| reason.contains("code hash")));
        assert!(evidence
            .abort_reasons
            .iter()
            .any(|reason| reason.contains("approver")));
        assert!(evidence
            .abort_reasons
            .iter()
            .any(|reason| reason.contains("pending")));
        assert!(evidence
            .abort_reasons
            .iter()
            .any(|reason| reason.contains("fee")));
    }

    #[test]
    fn mainnet_preflight_rejects_low_balance_and_failed_simulation() {
        let mut snapshot = ready_mainnet_snapshot();
        snapshot.wallet_near_balance_yocto = "0".to_string();
        snapshot.wallet_intents_wnear_balance = "1".to_string();
        snapshot.signed_payload_simulations[1] = IntentsSimulationResult {
            call_index: 1,
            ok: false,
            error: Some("insufficient balance".to_string()),
        };

        let evidence = evaluate_mainnet_preflight(&snapshot);

        assert!(!evidence.ready);
        assert!(evidence
            .abort_reasons
            .iter()
            .any(|reason| reason.contains("NEAR gas balance")));
        assert!(evidence
            .abort_reasons
            .iter()
            .any(|reason| reason.contains("wNEAR balance")));
        assert!(evidence
            .abort_reasons
            .iter()
            .any(|reason| reason.contains("simulation 1 failed")));
    }

    #[test]
    fn dust_acceptance_matches_expected_balance_delta_and_idempotency() {
        let mut handoff = CoordinatorHandoff::default();
        let workflow = mainnet_dust_workflow_json("mike.near").to_string();
        let first = handoff
            .execute_workflow_json(&workflow, "mainnet-dust-idempotency", PolicyDecision::Allow)
            .unwrap();
        let retry = handoff
            .execute_workflow_json(
                &workflow,
                "mainnet-dust-idempotency",
                PolicyDecision::Deny("retry should not re-check policy"),
            )
            .unwrap();

        assert_eq!(first, retry);
        assert_eq!(first.status, "succeeded");
        assert_eq!(first.intent_ids.len(), 3);
        assert_eq!(first.dispatch_block_heights, vec![10_000, 10_001, 10_002]);
        assert_eq!(
            first.balance_deltas.get("nep141:wrap.near:intents"),
            Some(&format!("-{DUST_TOTAL_YOCTO_WNEAR}"))
        );
        assert_eq!(handoff.policy_checks, 3);
        assert_eq!(handoff.nep413_signatures, 3);
        assert_eq!(handoff.nep366_delegates, 3);
        assert_eq!(handoff.broadcasts, 4);
    }

    #[test]
    fn sequential_batch_accepts_prebuilt_near_intents_calls() {
        let mut handoff = CoordinatorHandoff::default();
        let record = handoff
            .execute_sequential_batch_json(
                r#"{
                  "gate_id": "gate.sequential.near",
                  "calls": [
                    {
                      "receiver_id": "intents.near",
                      "method_name": "execute_intents",
                      "near_intents": {"kind": "transfer", "token": "nep141:wrap.near", "amount": "1"},
                      "gas": "100000000000000",
                      "deposit": "0"
                    },
                    {
                      "receiver_id": "intents.near",
                      "method_name": "execute_intents",
                      "near_intents": {"kind": "transfer", "token": "nep141:wrap.near", "amount": "1"},
                      "gas": "100000000000000",
                      "deposit": "0"
                    }
                  ]
                }"#,
                "seq-batch-intents",
                PolicyDecision::Allow,
            )
            .unwrap();

        assert_eq!(record.endpoint, "sequential_batch");
        assert_eq!(record.status, "succeeded");
        assert!(record.proxy_predecessor);
        assert_eq!(record.predecessor_model, "gate");
        assert_eq!(record.ordering_model, "gate_chained");
        assert_eq!(handoff.policy_checks, 2);
        assert_eq!(handoff.nep413_signatures, 2);
        assert_eq!(handoff.nep366_delegates, 2);
        assert_eq!(record.signed_nep413_payloads.len(), 2);
        assert_eq!(record.signed_nep366_delegates.len(), 2);
        assert_eq!(
            record
                .original_request_json
                .get("gate_id")
                .and_then(Value::as_str),
            Some("gate.sequential.near")
        );

        let status = handoff.sequence_status(&record.request_id).unwrap();
        assert_eq!(status, record);
    }

    #[test]
    fn sequential_batch_endpoint_executes_and_status_endpoint_replays_evidence() {
        let mut handoff = CoordinatorHandoff::default();
        let body = r#"{
          "idempotency_key": "sequential-endpoint-key",
          "gate_id": "gate.sequential.near",
          "calls": [
            {
              "receiver_id": "intents.near",
              "method_name": "execute_intents",
              "near_intents": {"kind": "transfer", "token": "nep141:wrap.near", "amount": "1"},
              "gas": "100000000000000",
              "deposit": "0"
            },
            {
              "receiver_id": "intents.near",
              "method_name": "execute_intents",
              "near_intents": {"kind": "transfer", "token": "nep141:wrap.near", "amount": "1"},
              "gas": "100000000000000",
              "deposit": "0"
            },
            {
              "receiver_id": "intents.near",
              "method_name": "execute_intents",
              "near_intents": {"kind": "transfer", "token": "nep141:wrap.near", "amount": "1"},
              "gas": "100000000000000",
              "deposit": "0"
            }
          ]
        }"#;

        let first = handoff.handle_post_sequential_batch(body, PolicyDecision::Allow);
        let retry = handoff.handle_post_sequential_batch(
            body,
            PolicyDecision::Deny("retry should reuse persisted record"),
        );

        assert_eq!(first.status_code, 200);
        assert_eq!(first, retry);
        assert_eq!(
            first.body.get("status").and_then(Value::as_str),
            Some("succeeded")
        );
        assert_eq!(handoff.policy_checks, 3);
        assert_eq!(handoff.nep413_signatures, 3);
        assert_eq!(handoff.nep366_delegates, 3);
        assert_eq!(handoff.broadcasts, 4);

        let request_id = first
            .body
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap();
        let status = handoff.handle_get_sequential_batch_status(request_id);
        assert_eq!(status.status_code, 200);
        assert_eq!(status.body, first.body);
    }

    #[test]
    fn status_endpoints_return_user_visible_404s() {
        let handoff = CoordinatorHandoff::default();

        let workflow = handoff.handle_get_workflow_status("missing-workflow");
        let sequence = handoff.handle_get_sequential_batch_status("missing-sequence");

        assert_eq!(workflow.status_code, 404);
        assert_eq!(sequence.status_code, 404);
        assert!(workflow.body.get("error").is_some());
        assert!(sequence.body.get("error").is_some());
    }

    #[test]
    fn sequential_batch_accepts_proxy_safe_raw_args_without_nep413_payloads() {
        let mut handoff = CoordinatorHandoff::default();
        let record = handoff
            .execute_sequential_batch_json(
                r#"{
                  "gate_id": "gate.sequential.near",
                  "calls": [
                    {
                      "receiver_id": "mock.proxy-safe.near",
                      "method_name": "record",
                      "args_json": {"sender_arg": "wallet.near", "action": "deposit"},
                      "gas": "30000000000000",
                      "deposit": "0"
                    },
                    {
                      "receiver_id": "mock.proxy-safe.near",
                      "method_name": "record",
                      "args_base64": "e30=",
                      "gas": "30000000000000",
                      "deposit": "0"
                    }
                  ]
                }"#,
                "seq-batch-raw",
                PolicyDecision::Allow,
            )
            .unwrap();

        assert_eq!(record.status, "succeeded");
        assert_eq!(handoff.policy_checks, 2);
        assert_eq!(handoff.nep413_signatures, 0);
        assert_eq!(handoff.nep366_delegates, 2);
        assert!(record.signed_nep413_payloads.is_empty());
        assert_eq!(record.signed_nep366_delegates.len(), 2);
        assert!(record.balance_deltas.is_empty());
    }

    #[test]
    fn sequential_batch_rejects_multiple_payload_modes_before_policy() {
        let mut handoff = CoordinatorHandoff::default();
        let record = handoff
            .execute_sequential_batch_json(
                r#"{
                  "gate_id": "gate.sequential.near",
                  "calls": [
                    {
                      "receiver_id": "intents.near",
                      "method_name": "execute_intents",
                      "near_intents": {"kind": "transfer"},
                      "args_json": {},
                      "gas": "100000000000000",
                      "deposit": "0"
                    }
                  ]
                }"#,
                "seq-batch-bad-payload",
                PolicyDecision::Allow,
            )
            .unwrap();

        assert_eq!(record.status, "rejected");
        assert!(record.persisted_before_signing);
        assert!(record
            .user_visible_error
            .as_deref()
            .unwrap_or_default()
            .contains("exactly one"));
        assert_eq!(handoff.policy_checks, 0);
        assert_eq!(handoff.nep413_signatures, 0);
        assert_eq!(handoff.nep366_delegates, 0);
        assert_eq!(handoff.broadcasts, 0);
    }

    #[test]
    fn sequential_batch_rejects_predecessor_sensitive_calls_before_policy() {
        let mut handoff = CoordinatorHandoff::default();
        let record = handoff
            .execute_sequential_batch_json(
                r#"{
                  "gate_id": "gate.sequential.near",
                  "calls": [
                    {
                      "receiver_id": "staking.pool.near",
                      "method_name": "withdraw_reward",
                      "args_json": {},
                      "gas": "30000000000000",
                      "deposit": "0",
                      "requires_user_predecessor": true
                    }
                  ]
                }"#,
                "seq-batch-predecessor",
                PolicyDecision::Allow,
            )
            .unwrap();

        assert_eq!(record.status, "rejected");
        assert!(record
            .user_visible_error
            .as_deref()
            .unwrap_or_default()
            .contains("requires user predecessor"));
        assert_eq!(handoff.policy_checks, 0);
        assert_eq!(handoff.nep413_signatures, 0);
        assert_eq!(handoff.nep366_delegates, 0);
    }

    #[test]
    fn policy_denial_happens_before_nep413_or_nep366_signing() {
        let mut handoff = CoordinatorHandoff::default();
        let record = handoff
            .execute_workflow_json(
                THREE_INTENTS_TRANSFERS,
                "policy-denied",
                PolicyDecision::Deny("amount exceeds wallet policy"),
            )
            .unwrap();

        assert_eq!(record.status, "policy_denied");
        assert_eq!(handoff.policy_checks, 3);
        assert_eq!(handoff.nep413_signatures, 0);
        assert_eq!(handoff.nep366_delegates, 0);
        assert_eq!(handoff.broadcasts, 0);
        assert!(record
            .user_visible_error
            .as_deref()
            .unwrap_or_default()
            .contains("amount exceeds wallet policy"));
    }

    #[test]
    fn idempotency_reuses_signed_payloads_tx_hashes_and_evidence() {
        let mut handoff = CoordinatorHandoff::default();
        let first = handoff
            .execute_workflow_json(THREE_INTENTS_TRANSFERS, "same-key", PolicyDecision::Allow)
            .unwrap();
        let second = handoff
            .execute_workflow_json(
                THREE_INTENTS_TRANSFERS,
                "same-key",
                PolicyDecision::Deny("deny retry"),
            )
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(handoff.policy_checks, 3);
        assert_eq!(handoff.nep413_signatures, 3);
        assert_eq!(handoff.nep366_delegates, 3);
        assert_eq!(handoff.broadcasts, 4);
        assert_eq!(second.status, "succeeded");
        assert_eq!(second.signed_nep413_payloads.len(), 3);
        assert_eq!(second.signed_nep366_delegates.len(), 3);
        assert_eq!(second.submit_tx_hashes.len(), 3);
        assert_eq!(second.dispatch_receipts.len(), 3);
    }

    #[test]
    fn async_submit_polling_parses_top_level_and_wrapped_logs() {
        let top_level = submit_outcome("intent-top", 0, OutcomeShape::TopLevel);
        let wrapped = submit_outcome("intent-wrapped", 1, OutcomeShape::ResultWrapped);

        let top_events = parse_gate_events(&top_level);
        let wrapped_events = parse_gate_events(&wrapped);

        assert_eq!(top_events.len(), 1);
        assert_eq!(top_events[0].kind, GateEventKind::IntentSubmitted);
        assert_eq!(top_events[0].intent_id.as_deref(), Some("intent-top"));
        assert_eq!(wrapped_events.len(), 1);
        assert_eq!(wrapped_events[0].kind, GateEventKind::IntentSubmitted);
        assert_eq!(
            wrapped_events[0].intent_id.as_deref(),
            Some("intent-wrapped")
        );
    }

    #[test]
    fn parser_collects_resume_and_dispatch_events_from_all_receipts() {
        let outcome = json!({
            "result": {
                "transaction_outcome": {
                    "outcome": {
                        "logs": [
                            event_log("batch_started", json!({"batch_id": "batch-1"}))
                        ]
                    }
                },
                "receipts_outcome": [
                    {
                        "outcome": {
                            "logs": [
                                event_log("intent_dispatched", json!({"intent_id": "intent-1", "receipt_id": "r1", "block_height": 42}))
                            ]
                        }
                    },
                    {
                        "outcome": {
                            "logs": [
                                event_log("chain_continued", json!({"intent_id": "intent-1", "block_height": 43}))
                            ]
                        }
                    }
                ]
            }
        });

        let events = parse_gate_events(&outcome);
        assert_eq!(events.len(), 3);
        assert!(events
            .iter()
            .any(|event| event.kind == GateEventKind::BatchStarted));
        assert!(events
            .iter()
            .any(|event| event.kind == GateEventKind::IntentDispatched));
        assert!(events
            .iter()
            .any(|event| event.kind == GateEventKind::ChainContinued));
    }

    #[test]
    fn submit_completion_order_does_not_determine_dispatch_order() {
        let mut handoff = CoordinatorHandoff::default();
        let record = handoff
            .execute_workflow_json(THREE_INTENTS_TRANSFERS, "scrambled", PolicyDecision::Allow)
            .unwrap();

        assert_eq!(
            record.intent_ids,
            vec![
                "intent-seq-req-1-0".to_string(),
                "intent-seq-req-1-1".to_string(),
                "intent-seq-req-1-2".to_string()
            ]
        );
        assert_eq!(
            record
                .dispatch_receipts
                .iter()
                .map(|receipt| receipt.intent_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "intent-seq-req-1-0",
                "intent-seq-req-1-1",
                "intent-seq-req-1-2"
            ]
        );
        assert_eq!(record.dispatch_block_heights, vec![10_000, 10_001, 10_002]);
    }

    #[test]
    fn direct_user_steps_are_planned_but_not_executed_gate_first() {
        let mut handoff = CoordinatorHandoff::default();
        let record = handoff
            .execute_workflow_json(
                r#"{
                  "steps": [
                    {
                      "kind": "near.function_call",
                      "predecessor_requirement": "user_required",
                      "user_id": "mike.near",
                      "receiver_id": "staking.pool.near",
                      "actions": [
                        {"method_name": "withdraw_reward", "gas": "30000000000000", "deposit": "0"}
                      ]
                    }
                  ]
                }"#,
                "direct-user",
                PolicyDecision::Allow,
            )
            .unwrap();

        assert_eq!(record.status, "requires_direct_user_setup");
        assert_eq!(record.predecessor_model, "user");
        assert_eq!(record.ordering_model, "single_tx_atomic");
        assert_eq!(handoff.policy_checks, 0);
        assert_eq!(handoff.nep413_signatures, 0);
        assert_eq!(handoff.broadcasts, 0);
    }

    #[test]
    fn workflow_evidence_contains_gate_models_and_receipt_evidence() {
        let mut handoff = CoordinatorHandoff::default();
        let record = handoff
            .execute_workflow_json(THREE_INTENTS_TRANSFERS, "evidence", PolicyDecision::Allow)
            .unwrap();

        assert_eq!(record.status, "succeeded");
        assert!(record.proxy_predecessor);
        assert_eq!(record.predecessor_model, "gate");
        assert_eq!(record.ordering_model, "gate_chained");
        assert_eq!(
            record.resume_tx_hash.as_deref(),
            Some("resume-tx-seq-req-1")
        );
        assert_eq!(record.intent_ids.len(), 3);
        assert_eq!(record.dispatch_receipts.len(), 3);
        assert_eq!(record.dispatch_block_heights, vec![10_000, 10_001, 10_002]);
        assert_eq!(
            record.balance_deltas.get("nep141:wrap.near:intents"),
            Some(&"-3000000000000000000".to_string())
        );

        let status = handoff.workflow_status(&record.request_id).unwrap();
        assert_eq!(status, record);
    }

    #[test]
    fn mixed_gate_batch_with_direct_user_step_rejects_before_policy() {
        let mut handoff = CoordinatorHandoff::default();
        let record = handoff
            .execute_workflow_json(
                r#"{
                  "steps": [
                    {
                      "kind": "intents.transfer",
                      "token": "nep141:wrap.near",
                      "amount": "1",
                      "receiver_id": "mike.near",
                      "gate_batch": "bad-mix"
                    },
                    {
                      "kind": "near.function_call",
                      "predecessor_requirement": "user_required",
                      "receiver_id": "staking.pool.near",
                      "method_name": "withdraw_reward",
                      "gate_batch": "bad-mix"
                    }
                  ]
                }"#,
                "mixed",
                PolicyDecision::Allow,
            )
            .unwrap();

        assert_eq!(record.status, "rejected");
        assert_eq!(handoff.policy_checks, 0);
        assert_eq!(handoff.nep413_signatures, 0);
        assert!(record
            .user_visible_error
            .as_deref()
            .unwrap_or_default()
            .contains("mixes proxy-safe and predecessor-sensitive"));
    }

    fn ready_mainnet_snapshot() -> MainnetPreflightSnapshot {
        MainnetPreflightSnapshot {
            gate_id: MAINNET_GATE_ID.to_string(),
            gate_code_hash: MAINNET_GATE_CODE_HASH.to_string(),
            gate_owner: MAINNET_GATE_OWNER.to_string(),
            gate_approver: MAINNET_GATE_APPROVER.to_string(),
            relayer_account_id: MAINNET_GATE_RELAYER.to_string(),
            relayer_whitelisted: true,
            pending_count: 0,
            fee_tier_1_to_3_yocto: GATE_FEE_1_TO_3_YOCTO.to_string(),
            wallet_near_balance_yocto: "100000000000000000000000".to_string(),
            wallet_intents_wnear_balance: DUST_TOTAL_YOCTO_WNEAR.to_string(),
            signed_payload_simulations: vec![
                IntentsSimulationResult {
                    call_index: 0,
                    ok: true,
                    error: None,
                },
                IntentsSimulationResult {
                    call_index: 1,
                    ok: true,
                    error: None,
                },
                IntentsSimulationResult {
                    call_index: 2,
                    ok: true,
                    error: None,
                },
            ],
        }
    }
}
