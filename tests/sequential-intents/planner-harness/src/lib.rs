use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowSpec {
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Deserialize)]
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
    pub route: Option<RouteHint>,
    #[serde(default)]
    pub requires_user_predecessor: bool,
    #[serde(default)]
    pub predecessor_requirement: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub gate_batch: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
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
    DirectUser,
    FundingSetup,
    Reject,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PredecessorModel {
    Gate,
    User,
    Wallet,
    View,
    None,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderingModel {
    GateChained,
    SingleTxAtomic,
    NormalTxOrView,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerError {
    InvalidJson(String),
}

impl fmt::Display for PlannerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(f, "invalid workflow JSON: {message}"),
        }
    }
}

impl std::error::Error for PlannerError {}

pub fn plan_workflow_json(input: &str) -> Result<PlanEvidence, PlannerError> {
    let spec: WorkflowSpec =
        serde_json::from_str(input).map_err(|err| PlannerError::InvalidJson(err.to_string()))?;
    Ok(plan_workflow(&spec))
}

pub fn plan_workflow(spec: &WorkflowSpec) -> PlanEvidence {
    let mut steps = spec
        .steps
        .iter()
        .map(classify_step)
        .collect::<Vec<StepEvidence>>();

    reject_mixed_gate_batches(spec, &mut steps);

    let accepted = steps.iter().all(|step| step.lane != Lane::Reject);
    PlanEvidence { accepted, steps }
}

fn classify_step(step: &WorkflowStep) -> StepEvidence {
    if let Some(kind) = step.kind.as_deref() {
        return classify_kind_step(step, kind);
    }

    if step.near_intents.is_some() {
        if is_gate_intents_call(step) {
            return evidence(step, Lane::GateProxy, None);
        }
        return evidence(
            step,
            Lane::Reject,
            Some("near_intents payloads are only valid for intents.near.execute_intents with zero deposit"),
        );
    }

    if step.route == Some(RouteHint::GateProxy) && step.requires_user_predecessor {
        return evidence(
            step,
            Lane::Reject,
            Some("gate_proxy cannot satisfy requires_user_predecessor"),
        );
    }

    if is_funding_setup(step) {
        return evidence(step, Lane::FundingSetup, None);
    }

    if step.requires_user_predecessor
        || step.route == Some(RouteHint::DirectUser)
        || is_staking_or_rewards(step)
    {
        return evidence(step, Lane::DirectUser, None);
    }

    if step.route == Some(RouteHint::FundingSetup) {
        return evidence(step, Lane::FundingSetup, None);
    }

    evidence(
        step,
        Lane::Reject,
        Some("step is ambiguous: mark it as near_intents, funding_setup, or requires_user_predecessor"),
    )
}

fn classify_kind_step(step: &WorkflowStep, kind: &str) -> StepEvidence {
    match kind {
        "intents.transfer" | "intents.swap" | "intents.execute_raw" => {
            evidence(step, Lane::GateProxy, None)
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
            spec.steps[*index].requires_user_predecessor
                || steps[*index].lane == Lane::DirectUser
                || (steps[*index].lane == Lane::Reject
                    && spec.steps[*index].route == Some(RouteHint::GateProxy))
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
        Lane::DirectUser => (
            PredecessorModel::User,
            OrderingModel::SingleTxAtomic,
            "direct_user",
        ),
        Lane::FundingSetup => (
            funding_predecessor_model(step),
            OrderingModel::NormalTxOrView,
            "setup",
        ),
        Lane::Reject => (PredecessorModel::None, OrderingModel::None, "reject"),
    };

    StepEvidence {
        lane,
        predecessor_model,
        ordering_model,
        receiver_id: step
            .receiver_id
            .clone()
            .or_else_placeholder(step.kind.as_deref()),
        method_name: step
            .method_name
            .clone()
            .or_else_placeholder(step.kind.as_deref()),
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

fn is_gate_intents_call(step: &WorkflowStep) -> bool {
    step.receiver_id == "intents.near"
        && step.method_name == "execute_intents"
        && step.deposit == "0"
}

fn is_funding_setup(step: &WorkflowStep) -> bool {
    if matches!(
        step.kind.as_deref(),
        Some(
            "funding.wrap_near"
                | "funding.intents_deposit"
                | "funding.balance_check"
                | "funding.storage_deposit"
        )
    ) {
        return true;
    }

    if step.route == Some(RouteHint::FundingSetup) {
        return true;
    }

    if step.receiver_id == "wrap.near" && step.method_name == "near_deposit" {
        return true;
    }

    if matches!(
        step.method_name.as_str(),
        "storage_deposit" | "storage_balance_of" | "ft_balance_of"
    ) {
        return true;
    }

    matches!(
        step.operation.as_deref(),
        Some("balance_read" | "funding_setup" | "intents_deposit" | "storage_setup" | "wrap_near")
    )
}

fn is_staking_or_rewards(step: &WorkflowStep) -> bool {
    matches!(
        step.operation.as_deref(),
        Some("staking_reward_withdraw" | "staking" | "rewards")
    )
}

fn funding_predecessor_model(step: &WorkflowStep) -> PredecessorModel {
    if matches!(step.operation.as_deref(), Some("balance_read"))
        || matches!(
            step.method_name.as_str(),
            "storage_balance_of" | "ft_balance_of"
        )
    {
        PredecessorModel::View
    } else {
        PredecessorModel::Wallet
    }
}

fn zero_deposit() -> String {
    "0".to_string()
}

trait PlaceholderString {
    fn or_else_placeholder(self, placeholder: Option<&str>) -> String;
}

impl PlaceholderString for String {
    fn or_else_placeholder(self, placeholder: Option<&str>) -> String {
        if self.is_empty() {
            placeholder.unwrap_or("").to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_execute_intents_calls_classify_as_gate_safe_batch() {
        let plan = plan_workflow_json(
            r#"{
              "steps": [
                {"receiver_id":"intents.near","method_name":"execute_intents","deposit":"0","near_intents":{"kind":"transfer"},"gate_batch":"batch-1"},
                {"receiver_id":"intents.near","method_name":"execute_intents","deposit":"0","near_intents":{"kind":"transfer"},"gate_batch":"batch-1"},
                {"receiver_id":"intents.near","method_name":"execute_intents","deposit":"0","near_intents":{"kind":"transfer"},"gate_batch":"batch-1"}
              ]
            }"#,
        )
        .unwrap();

        assert!(plan.accepted);
        assert_eq!(plan.steps.len(), 3);
        for step in plan.steps {
            assert_eq!(step.lane, Lane::GateProxy);
            assert_eq!(step.predecessor_model, PredecessorModel::Gate);
            assert_eq!(step.ordering_model, OrderingModel::GateChained);
            assert_eq!(step.policy_type, "call");
            assert!(step.reason.is_none());
        }
    }

    #[test]
    fn v1_intents_transfer_and_swap_classify_as_gate_proxy() {
        let plan = plan_workflow_json(
            r#"{
              "steps": [
                {"kind":"intents.transfer","token":"nep141:wrap.near","amount":"1","receiver_id":"mike.near"},
                {"kind":"intents.swap","token_in":"nep141:wrap.near","token_out":"nep141:btc.near","amount_in":"1"}
              ]
            }"#,
        )
        .unwrap();

        assert!(plan.accepted);
        assert_eq!(plan.steps.len(), 2);
        assert!(plan.steps.iter().all(|step| step.lane == Lane::GateProxy));
        assert!(plan
            .steps
            .iter()
            .all(|step| step.ordering_model == OrderingModel::GateChained));
    }

    #[test]
    fn wrap_near_deposit_classifies_as_funding_setup() {
        let plan = plan_workflow_json(
            r#"{
              "steps": [
                {"receiver_id":"wrap.near","method_name":"near_deposit","deposit":"1000000000000000000000000"}
              ]
            }"#,
        )
        .unwrap();

        let step = &plan.steps[0];
        assert!(plan.accepted);
        assert_eq!(step.lane, Lane::FundingSetup);
        assert_eq!(step.predecessor_model, PredecessorModel::Wallet);
        assert_ne!(step.lane, Lane::GateProxy);
    }

    #[test]
    fn v1_funding_steps_classify_as_setup() {
        let plan = plan_workflow_json(
            r#"{
              "steps": [
                {"kind":"funding.wrap_near","amount":"1000"},
                {"kind":"funding.intents_deposit","token":"wrap.near","amount":"1000"},
                {"kind":"funding.balance_check","token":"wrap.near"}
              ]
            }"#,
        )
        .unwrap();

        assert!(plan.accepted);
        assert!(plan
            .steps
            .iter()
            .all(|step| step.lane == Lane::FundingSetup));
    }

    #[test]
    fn staking_reward_withdraw_classifies_as_direct_user() {
        let plan = plan_workflow_json(
            r#"{
              "steps": [
                {
                  "receiver_id":"staking.pool.near",
                  "method_name":"withdraw_reward",
                  "requires_user_predecessor":true,
                  "operation":"staking_reward_withdraw"
                }
              ]
            }"#,
        )
        .unwrap();

        let step = &plan.steps[0];
        assert!(plan.accepted);
        assert_eq!(step.lane, Lane::DirectUser);
        assert_eq!(step.predecessor_model, PredecessorModel::User);
        assert_eq!(step.ordering_model, OrderingModel::SingleTxAtomic);
    }

    #[test]
    fn v1_user_required_near_function_call_classifies_as_direct_user() {
        let plan = plan_workflow_json(
            r#"{
              "steps": [
                {
                  "kind":"near.function_call",
                  "predecessor_requirement":"user_required",
                  "user_id":"mike.near",
                  "receiver_id":"staking.pool.near",
                  "actions":[
                    {"method_name":"withdraw_reward","gas":"30000000000000","deposit":"0"}
                  ]
                }
              ]
            }"#,
        )
        .unwrap();

        let step = &plan.steps[0];
        assert!(plan.accepted);
        assert_eq!(step.lane, Lane::DirectUser);
        assert_eq!(step.policy_type, "direct_user");
    }

    #[test]
    fn v1_ambiguous_near_function_call_is_rejected() {
        let plan = plan_workflow_json(
            r#"{
              "steps": [
                {
                  "kind":"near.function_call",
                  "predecessor_requirement":"wallet",
                  "receiver_id":"staking.pool.near",
                  "method_name":"withdraw_reward"
                }
              ]
            }"#,
        )
        .unwrap();

        let step = &plan.steps[0];
        assert!(!plan.accepted);
        assert_eq!(step.lane, Lane::Reject);
        assert!(step
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("user_required"));
    }

    #[test]
    fn gate_route_rejects_predecessor_sensitive_staking_call() {
        let plan = plan_workflow_json(
            r#"{
              "steps": [
                {
                  "receiver_id":"staking.pool.near",
                  "method_name":"withdraw_reward",
                  "route":"gate_proxy",
                  "requires_user_predecessor":true,
                  "operation":"staking_reward_withdraw"
                }
              ]
            }"#,
        )
        .unwrap();

        let step = &plan.steps[0];
        assert!(!plan.accepted);
        assert_eq!(step.lane, Lane::Reject);
        assert!(step
            .reason
            .as_deref()
            .unwrap_or_default()
            .contains("requires_user_predecessor"));
    }

    #[test]
    fn mixed_execute_intents_and_staking_gate_batch_is_rejected() {
        let plan = plan_workflow_json(
            r#"{
              "steps": [
                {"receiver_id":"intents.near","method_name":"execute_intents","deposit":"0","near_intents":{"kind":"transfer"},"gate_batch":"batch-1"},
                {
                  "receiver_id":"staking.pool.near",
                  "method_name":"withdraw_reward",
                  "requires_user_predecessor":true,
                  "operation":"staking_reward_withdraw",
                  "gate_batch":"batch-1"
                }
              ]
            }"#,
        )
        .unwrap();

        assert!(!plan.accepted);
        assert_eq!(plan.steps[0].lane, Lane::Reject);
        assert_eq!(plan.steps[1].lane, Lane::Reject);
        for step in &plan.steps {
            assert!(step
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("mixes proxy-safe and predecessor-sensitive"));
        }
    }

    #[test]
    fn evidence_serializes_all_required_fields_for_each_step() {
        let plan = plan_workflow_json(
            r#"{
              "steps": [
                {"receiver_id":"wrap.near","method_name":"near_deposit","deposit":"1"},
                {"receiver_id":"intents.near","method_name":"execute_intents","deposit":"0","near_intents":{"kind":"transfer"}}
              ]
            }"#,
        )
        .unwrap();

        let value = serde_json::to_value(plan).unwrap();
        let steps = value.get("steps").unwrap().as_array().unwrap();
        assert_eq!(steps.len(), 2);
        for step in steps {
            assert!(step.get("lane").is_some());
            assert!(step.get("predecessor_model").is_some());
            assert!(step.get("ordering_model").is_some());
            assert!(step.get("receiver_id").is_some());
            assert!(step.get("method_name").is_some());
            assert!(step.get("policy_type").is_some());
        }
    }
}
