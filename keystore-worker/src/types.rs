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
    /// Match the caller's account id against a regular expression.
    ///
    /// The pattern is matched against the WHOLE id (anchored `\A(?:…)\z` in
    /// [`compile_anchored_account_pattern`]), never a substring. Two regex
    /// facts still bite the unwary, so prefer [`AccessCondition::Whitelist`]
    /// for an exact set of accounts:
    ///   * `.` is a metacharacter — write `\.` for a literal dot, otherwise
    ///     `team.near` also admits `teamXnear`;
    ///   * a pattern is only as tight as it is written — `.*\.gov\.near`
    ///     admits every `*.gov.near`, which may be broader than intended.
    ///
    /// Example: `.*\.gov\.near` matches any `*.gov.near` account and only those.
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
    /// Require DAO membership (Sputnik v2 compatible)
    /// Checks if caller is member of specified role in DAO
    /// role: "council", "members", etc.
    DaoMember {
        dao_contract: String,
        role: String,
    },
}

/// Compile an [`AccessCondition::AccountPattern`] into a FULL-MATCH regex.
///
/// The pattern is wrapped in `\A(?:…)\z`, so it must match the ENTIRE caller
/// id. Without this, `regex::Regex::is_match` succeeds on any partial match:
/// a pattern `team\.near`, written as an exact check, would also admit
/// `xteam.near`, `team.near.attacker.near`, or — because `.` is a
/// metacharacter — `teamXnear`. That is a silent whitelist bypass that hands
/// one account's secrets to another, so the anchoring is a security boundary,
/// not a nicety.
///
/// `\A` / `\z` (absolute start / end of text) are deliberate over `^` / `$`,
/// but NOT for the reason that is usually given. In Perl, PCRE and Python `$`
/// also matches just before a trailing `\n`; in Rust's `regex` it does not —
/// measured, both `^team\.near$` and `\Ateam\.near\z` reject `"team.near\n"`.
///
/// The real difference is that `^` and `$` can be RE-POINTED by the pattern
/// itself: an owner writing `(?m)` turns them into line anchors, after which
/// `^team\.near$` is satisfied by the second line of `"evil.near\nteam.near"`.
/// `\A` and `\z` are absolute and no inline flag moves them, which is what
/// makes the full-match guarantee hold against a pattern its own author wrote.
/// See `test_owner_multiline_flag_cannot_defeat_absolute_anchors`.
///
/// The wrapping group `(?:…)` is required so a top-level alternation binds
/// correctly: `a|b` becomes `\A(?:a|b)\z`, not `\Aa|b\z` ("starts with a" OR
/// "ends with b"). An owner's own anchors, if present, are preserved —
/// `\A(?:^team\.near$)\z` is a harmless double anchor. A pattern that is
/// invalid on its own is invalid wrapped too, and the caller treats a compile
/// error as denial (fail-closed); wrapping a VALID pattern can never make it
/// invalid.
fn compile_anchored_account_pattern(pattern: &str) -> Result<regex::Regex, regex::Error> {
    regex::Regex::new(&format!(r"\A(?:{})\z", pattern))
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
                // Anchored full match — NOT `Regex::new(pattern).is_match`,
                // which matches a substring and silently turns an exact-looking
                // pattern into a whitelist bypass. See
                // [`compile_anchored_account_pattern`].
                match compile_anchored_account_pattern(pattern) {
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

            AccessCondition::DaoMember { dao_contract, role } => {
                let near_client = match near_client {
                    Some(client) => client,
                    None => {
                        tracing::warn!("DaoMember check requires NEAR client, but none provided");
                        return Ok(false);
                    }
                };

                // Check DAO membership for specified role
                let granted = near_client.check_dao_membership(dao_contract, caller, role).await?;

                tracing::debug!(
                    condition = "DaoMember",
                    dao_contract = %dao_contract,
                    role = %role,
                    caller = %caller,
                    granted = %granted,
                    "Validated DAO membership"
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
    async fn test_pattern_is_anchored_full_match() {
        // `team.near`, escaped, meant as an exact check. Unanchored `is_match`
        // used to admit every near-miss an attacker can register; anchoring
        // must reject all of them while still matching the real account.
        let condition = AccessCondition::AccountPattern {
            pattern: r"team\.near".to_string(),
        };
        assert!(condition.validate("team.near", None).await.unwrap());
        // Substring bypasses — all of these `is_match(caller)` accepted before.
        assert!(!condition.validate("xteam.near", None).await.unwrap());
        assert!(!condition.validate("team.near.attacker.near", None).await.unwrap());
        assert!(!condition.validate("team.nearx", None).await.unwrap());
        // A trailing newline must not satisfy the anchor. (Rust's `$` would
        // also reject this one — see the note on `\A`/`\z` above for what the
        // absolute anchors actually buy.)
        assert!(!condition.validate("team.near\n", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_unescaped_dot_matches_one_char_but_never_a_substring() {
        // Anchoring does NOT fix the `.`-is-a-metachar footgun (that needs the
        // owner to escape it); it only removes the SUBSTRING bypass. This pins
        // the anchored behaviour so a refactor cannot silently un-anchor.
        let condition = AccessCondition::AccountPattern {
            pattern: r"team.near".to_string(), // unescaped dot — owner mistake
        };
        assert!(condition.validate("team.near", None).await.unwrap());
        // `.` still matches one arbitrary char (the documented footgun)...
        assert!(condition.validate("teamXnear", None).await.unwrap());
        // ...but only as a whole-id match, never as a substring of a longer id.
        assert!(!condition.validate("teamXnear.attacker.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_pattern_top_level_alternation_is_grouped() {
        // `a|b` must anchor as a whole: `\A(?:a|b)\z`, not `\Aa|b\z`, which
        // would mean "starts with a" OR "ends with b" — a bypass on both sides.
        let condition = AccessCondition::AccountPattern {
            pattern: r"alice\.near|bob\.near".to_string(),
        };
        assert!(condition.validate("alice.near", None).await.unwrap());
        assert!(condition.validate("bob.near", None).await.unwrap());
        assert!(!condition.validate("alice.near.evil.near", None).await.unwrap());
        assert!(!condition.validate("evil.bob.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_empty_pattern_denies_instead_of_granting_everyone() {
        // Latent bug the anchoring incidentally closes: `Regex::new("")` matches
        // ANY input, so an empty pattern used to grant every caller. Anchored,
        // `\A(?:)\z` matches only the empty string, so a real account is denied.
        let condition = AccessCondition::AccountPattern { pattern: String::new() };
        assert!(!condition.validate("anyone.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_metachar_injection_cannot_escape_the_anchors() {
        // A pattern that injects its own parens can restructure the groups but
        // never remove `\A`/`\z`: `a)(b` compiles to `\A(?:a)(b)\z`, still a
        // full match of exactly "ab" — no substring bypass survives.
        let condition = AccessCondition::AccountPattern { pattern: "a)(b".to_string() };
        assert!(condition.validate("ab", None).await.unwrap());
        assert!(!condition.validate("xabx", None).await.unwrap());
        // A pattern that unbalances the wrapper is invalid → fail-closed deny.
        let broken = AccessCondition::AccountPattern { pattern: ")".to_string() };
        assert!(!broken.validate("anything.near", None).await.unwrap());
    }

    #[tokio::test]
    async fn test_owner_multiline_flag_cannot_defeat_absolute_anchors() {
        // `\A`/`\z` are absolute, unlike `^`/`$`, so even an owner-injected
        // `(?m)` (which only re-points `^`/`$` at line boundaries) cannot let a
        // second line satisfy the match across an embedded newline.
        let condition = AccessCondition::AccountPattern {
            pattern: r"(?m)^team\.near$".to_string(),
        };
        assert!(!condition.validate("evil.near\nteam.near", None).await.unwrap());
        assert!(condition.validate("team.near", None).await.unwrap());
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

