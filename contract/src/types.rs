use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::{near, AccountId, NearToken};

// V1 enums for backward compatibility
#[derive(PartialEq, Debug, Clone, Copy)]
#[near(serializers=[borsh, json])]
pub enum LogicOperatorV1 {
    And,
    Or,
}

#[derive(PartialEq, Debug, Clone, Copy)]
#[near(serializers=[borsh, json])]
pub enum ComparisonOperatorV1 {
    Gte, // >=
    Lte, // <=
    Gt,  // >
    Lt,  // <
    Eq,  // ==
    Ne,  // !=
}

/// Access control conditions for secrets (V1)
/// Supports complex logic combinations with regex patterns
#[derive(Clone, Debug, PartialEq)]
#[near(serializers=[borsh, json])]
pub enum AccessConditionV1 {
    /// Logical combination of conditions
    Logic {
        operator: LogicOperatorV1,
        conditions: Vec<AccessConditionV1>,
    },
    /// Logical NOT
    Not {
        condition: Box<AccessConditionV1>,
    },
    /// Allow all accounts
    AllowAll,
    /// Whitelist specific accounts
    Whitelist {
        accounts: Vec<AccountId>,
    },
    /// Match account name pattern (regex)
    /// Example: ".*\\.gov\\.near" matches all .gov.near accounts
    AccountPattern {
        pattern: String,
    },
    /// Require minimum NEAR balance
    NearBalance {
        operator: ComparisonOperatorV1,
        value: NearToken,
    },
    /// Require minimum fungible token balance
    FtBalance {
        contract: AccountId,
        operator: ComparisonOperatorV1,
        value: NearToken,
    },
    /// Require NFT ownership
    NftOwned {
        contract: AccountId,
    },
}

// Versioned enums for future upgrades
#[derive(Clone)]
#[near(serializers=[borsh])]
pub enum VersionedLogicOperator {
    V1(LogicOperatorV1),
}

#[derive(Clone)]
#[near(serializers=[borsh])]
pub enum VersionedComparisonOperator {
    V1(ComparisonOperatorV1),
}

#[derive(Clone)]
#[near(serializers=[borsh])]
pub enum VersionedAccessCondition {
    V1(AccessConditionV1),
}

// Migration traits and implementations
impl From<VersionedLogicOperator> for LogicOperatorV1 {
    fn from(versioned: VersionedLogicOperator) -> Self {
        match versioned {
            VersionedLogicOperator::V1(operator) => operator,
        }
    }
}

impl From<VersionedComparisonOperator> for ComparisonOperatorV1 {
    fn from(versioned: VersionedComparisonOperator) -> Self {
        match versioned {
            VersionedComparisonOperator::V1(operator) => operator,
        }
    }
}

impl From<VersionedAccessCondition> for AccessConditionV1 {
    fn from(versioned: VersionedAccessCondition) -> Self {
        match versioned {
            VersionedAccessCondition::V1(condition) => condition,
        }
    }
}

// Conversion from current types to versioned
impl From<LogicOperatorV1> for VersionedLogicOperator {
    fn from(operator: LogicOperatorV1) -> Self {
        VersionedLogicOperator::V1(operator)
    }
}

impl From<ComparisonOperatorV1> for VersionedComparisonOperator {
    fn from(operator: ComparisonOperatorV1) -> Self {
        VersionedComparisonOperator::V1(operator)
    }
}

impl From<AccessConditionV1> for VersionedAccessCondition {
    fn from(condition: AccessConditionV1) -> Self {
        VersionedAccessCondition::V1(condition)
    }
}

// Current type aliases (point to latest versions)
pub type LogicOperator = LogicOperatorV1;
pub type ComparisonOperator = ComparisonOperatorV1;
pub type AccessCondition = AccessConditionV1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_condition_serialization() {
        let condition = AccessCondition::AllowAll;
        let json = near_sdk::serde_json::to_string(&condition).unwrap();
        assert!(json.contains("AllowAll"));
    }

    #[test]
    fn test_whitelist_condition() {
        let condition = AccessCondition::Whitelist {
            accounts: vec!["alice.near".parse().unwrap(), "bob.near".parse().unwrap()],
        };
        assert_eq!(condition, condition.clone());
    }

    #[test]
    fn test_pattern_condition() {
        let condition = AccessCondition::AccountPattern {
            pattern: String::from(".*\\.gov\\.near"),
        };
        let json = near_sdk::serde_json::to_string(&condition).unwrap();
        assert!(json.contains("AccountPattern"));
        assert!(json.contains("\\\\.gov\\\\.near")); // escaped in JSON
    }

    #[test]
    fn test_logic_and_condition() {
        let condition = AccessCondition::Logic {
            operator: LogicOperator::And,
            conditions: vec![
                AccessCondition::AccountPattern {
                    pattern: String::from(".*\\.near"),
                },
                AccessCondition::NearBalance {
                    operator: ComparisonOperator::Gte,
                    value: NearToken::from_near(10),
                },
            ],
        };
        assert_eq!(condition, condition.clone());
    }

    #[test]
    fn test_not_condition() {
        let condition = AccessCondition::Not {
            condition: Box::new(AccessCondition::AccountPattern {
                pattern: String::from(".*\\.blocked\\.near"),
            }),
        };
        assert_eq!(condition, condition.clone());
    }

    #[test]
    fn test_versioned_conversion() {
        let original = AccessCondition::AllowAll;
        let versioned: VersionedAccessCondition = original.clone().into();
        let back: AccessCondition = versioned.into();
        assert_eq!(original, back);
    }
}
