//! Access control types for validating secrets access
//!
//! Adapted from contract types for use in keystore validation

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicOperator {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOperator {
    #[serde(rename = "Gte")]
    Gte, // >=
    #[serde(rename = "Lte")]
    Lte, // <=
    #[serde(rename = "Gt")]
    Gt, // >
    #[serde(rename = "Lt")]
    Lt, // <
    #[serde(rename = "Eq")]
    Eq, // ==
    #[serde(rename = "Ne")]
    Ne, // !=
}

/// Access control conditions for secrets
/// Note: Matches NEAR SDK adjacently tagged enum format
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AccessCondition {
    /// Logical combination of conditions
    Logic {
        operator: LogicOperator,
        conditions: Vec<AccessCondition>,
    },
    /// Logical NOT
    Not {
        condition: Box<AccessCondition>,
    },
    /// Allow all accounts (no restrictions)
    AllowAll,
    /// Whitelist specific accounts
    Whitelist {
        accounts: Vec<String>,
    },
    /// Match account name pattern (regex)
    /// Example: ".*\\.gov\\.near" matches all .gov.near accounts
    AccountPattern {
        pattern: String,
    },
    /// Require minimum NEAR balance (in yoctoNEAR)
    NearBalance {
        operator: ComparisonOperator,
        value: String, // u128 as string
    },
    /// Require minimum fungible token balance
    FtBalance {
        contract: String,
        operator: ComparisonOperator,
        value: String, // u128 as string
    },
    /// Require NFT ownership
    /// token_id: None = any token from this contract
    /// token_id: Some("123") = specific token ID
    NftOwned {
        contract: String,
        token_id: Option<String>,
    },
}

impl AccessCondition {
    /// Validate access condition against caller account
    ///
    /// Returns Ok(true) if access granted, Ok(false) if denied
    /// Returns Err if validation failed (e.g. invalid regex, RPC error)
    pub async fn validate(&self, caller: &str, near_client: Option<&crate::near::NearClient>) -> anyhow::Result<bool> {
        match self {
            AccessCondition::AllowAll => {
                tracing::debug!("AllowAll condition - access granted");
                Ok(true)
            }

            AccessCondition::Whitelist { accounts } => {
                let granted = accounts.iter().any(|acc| acc == caller);
                tracing::debug!(
                    condition = "Whitelist",
                    caller = %caller,
                    granted = %granted,
                    "Validated whitelist"
                );
                Ok(granted)
            }

            AccessCondition::AccountPattern { pattern } => {
                match regex::Regex::new(pattern) {
                    Ok(re) => {
                        let granted = re.is_match(caller);
                        tracing::debug!(
                            condition = "AccountPattern",
                            pattern = %pattern,
                            caller = %caller,
                            granted = %granted,
                            "Validated account pattern"
                        );
                        Ok(granted)
                    }
                    Err(e) => {
                        tracing::warn!(
                            pattern = %pattern,
                            error = %e,
                            "Invalid regex pattern in AccessCondition"
                        );
                        // Invalid regex = deny access (fail-safe)
                        Ok(false)
                    }
                }
            }

            AccessCondition::Logic { operator, conditions } => {
                match operator {
                    LogicOperator::And => {
                        // All conditions must pass
                        for condition in conditions {
                            let fut = Box::pin(condition.validate(caller, near_client));
                            if !fut.await? {
                                tracing::debug!("Logic::And - condition failed");
                                return Ok(false);
                            }
                        }
                        tracing::debug!("Logic::And - all conditions passed");
                        Ok(true)
                    }
                    LogicOperator::Or => {
                        // At least one condition must pass
                        for condition in conditions {
                            let fut = Box::pin(condition.validate(caller, near_client));
                            if fut.await? {
                                tracing::debug!("Logic::Or - condition passed");
                                return Ok(true);
                            }
                        }
                        tracing::debug!("Logic::Or - no conditions passed");
                        Ok(false)
                    }
                }
            }

            AccessCondition::Not { condition } => {
                let fut = Box::pin(condition.validate(caller, near_client));
                let result = fut.await?;
                tracing::debug!(
                    inner_result = %result,
                    negated = %(!result),
                    "Logic::Not"
                );
                Ok(!result)
            }

            AccessCondition::NearBalance { operator, value } => {
                let near_client = match near_client {
                    Some(client) => client,
                    None => {
                        tracing::warn!("NearBalance check requires NEAR client, but none provided");
                        return Ok(false);
                    }
                };

                // Parse required balance
                let required_balance: u128 = value.parse()
                    .map_err(|e| anyhow::anyhow!("Invalid balance value: {}", e))?;

                // Get actual balance
                let actual_balance = near_client.get_account_balance(caller).await?;

                // Compare
                let granted = Self::compare_values(actual_balance, *operator, required_balance);

                tracing::debug!(
                    condition = "NearBalance",
                    caller = %caller,
                    actual = %actual_balance,
                    required = %required_balance,
                    operator = ?operator,
                    granted = %granted,
                    "Validated NEAR balance"
                );

                Ok(granted)
            }

            AccessCondition::FtBalance { contract, operator, value } => {
                let near_client = match near_client {
                    Some(client) => client,
                    None => {
                        tracing::warn!("FtBalance check requires NEAR client, but none provided");
                        return Ok(false);
                    }
                };

                // Parse required balance
                let required_balance: u128 = value.parse()
                    .map_err(|e| anyhow::anyhow!("Invalid balance value: {}", e))?;

                // Get actual FT balance
                let actual_balance = near_client.get_ft_balance(contract, caller).await?;

                // Compare
                let granted = Self::compare_values(actual_balance, *operator, required_balance);

                tracing::debug!(
                    condition = "FtBalance",
                    contract = %contract,
                    caller = %caller,
                    actual = %actual_balance,
                    required = %required_balance,
                    operator = ?operator,
                    granted = %granted,
                    "Validated FT balance"
                );

                Ok(granted)
            }

            AccessCondition::NftOwned { contract, token_id } => {
                let near_client = match near_client {
                    Some(client) => client,
                    None => {
                        tracing::warn!("NftOwned check requires NEAR client, but none provided");
                        return Ok(false);
                    }
                };

                // Check NFT ownership (specific token or any token)
                let granted = near_client.check_nft_ownership(contract, caller, token_id.as_deref()).await?;

                tracing::debug!(
                    condition = "NftOwned",
                    contract = %contract,
                    caller = %caller,
                    token_id = ?token_id,
                    granted = %granted,
                    "Validated NFT ownership"
                );

                Ok(granted)
            }
        }
    }

    /// Compare two u128 values using the given operator
    fn compare_values(actual: u128, operator: ComparisonOperator, required: u128) -> bool {
        match operator {
            ComparisonOperator::Gte => actual >= required,
            ComparisonOperator::Lte => actual <= required,
            ComparisonOperator::Gt => actual > required,
            ComparisonOperator::Lt => actual < required,
            ComparisonOperator::Eq => actual == required,
            ComparisonOperator::Ne => actual != required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_allow_all() {
        let condition = AccessCondition::AllowAll;
        assert!(condition.validate("anyone.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_whitelist_allowed() {
        let condition = AccessCondition::Whitelist {
            accounts: vec!["alice.near".to_string(), "bob.near".to_string()],
        };
        assert!(condition.validate("alice.near", None).await.unwrap());
        assert!(condition.validate("bob.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_whitelist_denied() {
        let condition = AccessCondition::Whitelist {
            accounts: vec!["alice.near".to_string()],
        };
        assert!(!condition.validate("bob.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_pattern_match() {
        let condition = AccessCondition::AccountPattern {
            pattern: r".*\.gov\.near".to_string(),
        };
        assert!(condition.validate("treasury.gov.near", None).await.unwrap());
        assert!(!condition.validate("alice.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_pattern_invalid_regex() {
        let condition = AccessCondition::AccountPattern {
            pattern: "[invalid".to_string(), // unclosed bracket
        };
        // Invalid regex should deny access
        assert!(!condition.validate("alice.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_logic_and_pass() {
        let condition = AccessCondition::Logic {
            operator: LogicOperator::And,
            conditions: vec![
                AccessCondition::AccountPattern {
                    pattern: r".*\.near".to_string(),
                },
                AccessCondition::Whitelist {
                    accounts: vec!["alice.near".to_string()],
                },
            ],
        };
        assert!(condition.validate("alice.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_logic_and_fail() {
        let condition = AccessCondition::Logic {
            operator: LogicOperator::And,
            conditions: vec![
                AccessCondition::AccountPattern {
                    pattern: r".*\.near".to_string(),
                },
                AccessCondition::Whitelist {
                    accounts: vec!["bob.near".to_string()],
                },
            ],
        };
        assert!(!condition.validate("alice.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_logic_or_pass() {
        let condition = AccessCondition::Logic {
            operator: LogicOperator::Or,
            conditions: vec![
                AccessCondition::Whitelist {
                    accounts: vec!["bob.near".to_string()],
                },
                AccessCondition::AccountPattern {
                    pattern: r"alice\..*".to_string(),
                },
            ],
        };
        assert!(condition.validate("alice.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_logic_not() {
        let condition = AccessCondition::Not {
            condition: Box::new(AccessCondition::Whitelist {
                accounts: vec!["blocked.near".to_string()],
            }),
        };
        assert!(condition.validate("alice.near", None).await.unwrap());
        assert!(!condition.validate("blocked.near", None).await.unwrap());
    }
}

#[cfg(test)]
mod near_sdk_format_tests {
    use super::*;

    #[test]
    fn test_parse_allow_all_from_contract() {
        // NEAR SDK returns unit variants as simple strings
        let json = r#""AllowAll""#;
        let parsed: AccessCondition = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, AccessCondition::AllowAll);
    }

    #[test]
    fn test_parse_whitelist_from_contract() {
        // NEAR SDK returns struct variants as adjacently tagged
        let json = r#"{"Whitelist":{"accounts":["alice.near","bob.near"]}}"#;
        let parsed: AccessCondition = serde_json::from_str(json).unwrap();
        match parsed {
            AccessCondition::Whitelist { accounts } => {
                assert_eq!(accounts, vec!["alice.near", "bob.near"]);
            }
            _ => panic!("Expected Whitelist variant"),
        }
    }
}
