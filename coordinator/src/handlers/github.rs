use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use crate::github::parse_github_repo;
use crate::AppState;

const BRANCH_CACHE_TTL: usize = 7 * 24 * 60 * 60; // 7 days in seconds

#[derive(Debug, Deserialize)]
pub struct ResolveBranchQuery {
    pub repo: String,
    pub commit: String,
}

#[derive(Debug, Serialize)]
pub struct ResolveBranchResponse {
    pub branch: Option<String>,
    pub repo_normalized: String,
    pub cached: bool,
}

/// GET /github/resolve-branch?repo=...&commit=...
///
/// Resolves which branch contains a specific commit hash.
/// Results are cached in Redis for 7 days.
///
/// Returns:
/// - 200: { "branch": "main", "repo_normalized": "github.com/owner/repo", "cached": true }
/// - 400: Invalid repo format
/// - 404: Commit not found or branch could not be determined
/// - 500: Internal error
pub async fn resolve_branch(
    Query(query): Query<ResolveBranchQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Normalize repo URL
    let normalized_repo = parse_github_repo(&query.repo)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid repo format: {}", e)))?;

    // Check Redis cache first
    let cache_key = format!("branch_cache:{}:{}", normalized_repo, query.commit);

    let mut redis_conn = state.redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis error: {}", e)))?;

    // Try to get from cache
    let cached_branch: Option<String> = redis_conn
        .get(&cache_key)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Redis get error: {}", e)))?;

    if let Some(branch) = cached_branch {
        tracing::debug!(
            "Branch cache HIT: repo={}, commit={}, branch={}",
            normalized_repo,
            query.commit,
            branch
        );

        return Ok((
            StatusCode::OK,
            Json(ResolveBranchResponse {
                branch: Some(branch),
                repo_normalized: normalized_repo,
                cached: true,
            }),
        ));
    }

    // Cache miss - check if commit is SHA or branch name
    let is_full_sha = query.commit.len() == 40 && query.commit.chars().all(|c| c.is_ascii_hexdigit());
    let is_short_sha = (query.commit.len() == 7 || query.commit.len() == 8)
        && query.commit.chars().all(|c| c.is_ascii_hexdigit());

    let branch = if is_full_sha || is_short_sha {
        // It's a SHA commit - query GitHub API
        tracing::debug!(
            "Branch cache MISS: repo={}, commit={} looks like SHA - querying GitHub API",
            normalized_repo,
            query.commit
        );

        fetch_branch_from_github(&normalized_repo, &query.commit)
            .await
            .map_err(|e| (StatusCode::NOT_FOUND, format!("Failed to resolve branch: {}", e)))?
    } else {
        // It's a branch name - return as-is without API call
        tracing::debug!(
            "Branch cache MISS: repo={}, commit={} looks like branch name - no API call",
            normalized_repo,
            query.commit
        );
        Some(query.commit.clone())
    };

    // Store in cache
    if let Some(ref b) = branch {
        let _: () = redis_conn
            .set_ex(&cache_key, b, BRANCH_CACHE_TTL as u64)
            .await
            .map_err(|e| {
                tracing::warn!("Failed to cache branch result: {}", e);
                // Don't fail the request if caching fails
            })
            .unwrap_or(());

        tracing::info!(
            "Cached branch resolution: repo={}, commit={}, branch={}, ttl={}s",
            normalized_repo,
            query.commit,
            b,
            BRANCH_CACHE_TTL
        );
    }

    Ok((
        StatusCode::OK,
        Json(ResolveBranchResponse {
            branch,
            repo_normalized: normalized_repo,
            cached: false,
        }),
    ))
}

/// Fetch branch name from GitHub API for a specific commit
async fn fetch_branch_from_github(
    normalized_repo: &str,
    commit: &str,
) -> Result<Option<String>, String> {
    // Parse repo: "github.com/owner/repo" or "gitlab.com/owner/repo"
    let parts: Vec<&str> = normalized_repo.split('/').collect();
    if parts.len() != 3 {
        return Err(format!("Invalid normalized repo format: {}", normalized_repo));
    }

    let host = parts[0];
    let owner = parts[1];
    let repo = parts[2];

    // Currently only support GitHub
    if host != "github.com" {
        return Err(format!("Only github.com is supported, got: {}", host));
    }

    // GitHub API: List branches containing commit
    // This works for any commit, not just HEAD
    let url = format!(
        "https://api.github.com/repos/{}/{}/commits/{}/branches-where-head?per_page=100",
        owner, repo, commit
    );

    let client = reqwest::Client::new();

    // First try branches-where-head (fast, only for HEAD commits)
    let response = client
        .get(&url)
        .header("User-Agent", "offchainvm-coordinator")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if response.status() == 404 {
        return Ok(None); // Commit not found
    }

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "GitHub API error {}: {}",
            status,
            body.chars().take(200).collect::<String>()
        ));
    }

    #[derive(Deserialize)]
    struct BranchInfo {
        name: String,
    }

    let branches: Vec<BranchInfo> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))?;

    // If commit is HEAD of any branch, prefer main/master
    if !branches.is_empty() {
        let main_branch = branches
            .iter()
            .find(|b| b.name == "main" || b.name == "master")
            .map(|b| b.name.clone());

        if let Some(branch) = main_branch {
            return Ok(Some(branch));
        }

        // Otherwise return first branch
        return Ok(branches.first().map(|b| b.name.clone()));
    }

    // Commit exists but not HEAD - need to search all branches
    // This is slower but works for any commit in history
    tracing::debug!(
        "Commit {} not HEAD of any branch, searching all branches",
        commit
    );

    let branches_url = format!(
        "https://api.github.com/repos/{}/{}/branches?per_page=100",
        owner, repo
    );

    let branches_response = client
        .get(&branches_url)
        .header("User-Agent", "offchainvm-coordinator")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch branches list: {}", e))?;

    if !branches_response.status().is_success() {
        tracing::warn!("Failed to fetch branches list, assuming 'main'");
        return Ok(Some("main".to_string()));
    }

    let all_branches: Vec<BranchInfo> = branches_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse branches response: {}", e))?;

    // Check each branch to see if it contains the commit
    for branch in &all_branches {
        if branch.name == "main" || branch.name == "master" {
            // Check if main/master contains this commit
            let compare_url = format!(
                "https://api.github.com/repos/{}/{}/compare/{}...{}",
                owner, repo, commit, branch.name
            );

            let compare_response = client
                .get(&compare_url)
                .header("User-Agent", "offchainvm-coordinator")
                .header("Accept", "application/vnd.github.v3+json")
                .send()
                .await;

            if let Ok(resp) = compare_response {
                if resp.status().is_success() {
                    #[derive(Deserialize)]
                    struct CompareInfo {
                        status: String, // "ahead", "behind", "identical", "diverged"
                    }

                    if let Ok(compare_data) = resp.json::<CompareInfo>().await {
                        // If status is "behind" or "identical", commit is in this branch
                        if compare_data.status == "behind" || compare_data.status == "identical" {
                            return Ok(Some(branch.name.clone()));
                        }
                    }
                }
            }
        }
    }

    // If main/master doesn't contain it, check other branches
    for branch in &all_branches {
        if branch.name != "main" && branch.name != "master" {
            let compare_url = format!(
                "https://api.github.com/repos/{}/{}/compare/{}...{}",
                owner, repo, commit, branch.name
            );

            let compare_response = client
                .get(&compare_url)
                .header("User-Agent", "offchainvm-coordinator")
                .header("Accept", "application/vnd.github.v3+json")
                .send()
                .await;

            if let Ok(resp) = compare_response {
                if resp.status().is_success() {
                    #[derive(Deserialize)]
                    struct CompareInfo {
                        status: String,
                    }

                    if let Ok(compare_data) = resp.json::<CompareInfo>().await {
                        if compare_data.status == "behind" || compare_data.status == "identical" {
                            return Ok(Some(branch.name.clone()));
                        }
                    }
                }
            }
        }
    }

    // Couldn't find any branch containing this commit - assume main
    tracing::warn!(
        "Commit {} not found in any branch, assuming 'main'",
        commit
    );
    Ok(Some("main".to_string()))
}

/// POST /secrets/pubkey
/// Request: { "repo": "...", "branch": "...", "owner": "...", "secrets_json": "{...}" }
///
/// Get public key for encrypting secrets for a specific repository, branch, and owner.
/// Keystore validates secrets JSON and rejects reserved keywords before returning pubkey.
/// This endpoint proxies the request to the keystore worker.
///
/// Query parameters:
/// - repo: Repository URL (will be normalized)
/// - branch: Optional branch name (omit for all branches)
/// - owner: NEAR account ID that will own these secrets (REQUIRED for security)
///
/// Security: Owner is included in seed to prevent secret reuse attacks.
/// Different owners get different encryption keys for the same repo.
/// Seed format: "repo:owner[:branch]" (branch is last for unambiguity)
///
/// Returns:
/// - 200: { "repo_normalized": "github.com/owner/repo", "branch": "main", "owner": "alice.near", "pubkey": "hex..." }
/// - 400: Invalid repo format, missing owner, or keystore not configured
/// - 502: Keystore error
#[derive(Debug, Deserialize)]
pub struct GetPubkeyRequest {
    pub repo: String,
    pub branch: Option<String>,
    pub owner: String, // NEAR account ID (required)
    pub secrets_json: String, // Secrets to validate before encryption
}

#[derive(Debug, Serialize)]
pub struct GetPubkeyResponse {
    pub repo_normalized: String,
    pub branch: Option<String>,
    pub owner: String,
    pub pubkey: String, // hex-encoded public key
}

pub async fn get_secrets_pubkey(
    State(state): State<AppState>,
    Json(req): Json<GetPubkeyRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Check if keystore is configured
    let keystore_url = state
        .config
        .keystore_base_url
        .as_ref()
        .ok_or((
            StatusCode::BAD_REQUEST,
            "Keystore is not configured".to_string(),
        ))?;

    // Validate owner account ID
    if req.owner.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Owner account ID is required".to_string(),
        ));
    }

    // Normalize repo URL
    let normalized_repo = parse_github_repo(&req.repo)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid repo format: {}", e)))?;

    // Build seed for keystore: repo:owner[:branch]
    // This prevents secret reuse attacks (different owners get different keys)
    let seed = if let Some(ref branch) = req.branch {
        format!("{}:{}:{}", normalized_repo, req.owner, branch)
    } else {
        format!("{}:{}", normalized_repo, req.owner)
    };

    tracing::info!(
        "🔐 ENCRYPTION SEED (coordinator): repo_normalized={}, owner={}, branch={:?}, seed={}",
        normalized_repo,
        req.owner,
        req.branch,
        seed
    );

    // Call keystore to get pubkey (POST with seed and secrets_json for validation)
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "seed": seed,
        "secrets_json": req.secrets_json,
    });

    let mut request_builder = client
        .post(format!("{}/pubkey", keystore_url))
        .json(&payload);

    // Add Authorization header if token is configured
    if let Some(ref token) = state.config.keystore_auth_token {
        tracing::debug!("Adding keystore auth token (length: {})", token.len());
        request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
    } else {
        tracing::warn!("⚠️ KEYSTORE_AUTH_TOKEN not configured - keystore request will fail!");
    }

    let keystore_response = request_builder
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("Failed to connect to keystore: {}", e),
            )
        })?;

    if !keystore_response.status().is_success() {
        let status = keystore_response.status();
        let body = keystore_response.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!(
                "Keystore returned error {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            ),
        ));
    }

    #[derive(Deserialize)]
    struct KeystoreResponse {
        pubkey: String,
    }

    let keystore_data: KeystoreResponse = keystore_response.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Failed to parse keystore response: {}", e),
        )
    })?;

    Ok((
        StatusCode::OK,
        Json(GetPubkeyResponse {
            repo_normalized: normalized_repo,
            branch: req.branch,
            owner: req.owner,
            pubkey: keystore_data.pubkey,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires internet access - run with: cargo test -- --ignored
    async fn test_fetch_branch_real_github() {
        // Using current HEAD of main branch (update this SHA periodically if test fails)
        let result = fetch_branch_from_github(
            "github.com/zavodil/random-ark",
            "bdd6fbd0cd36c0a6ffcb713535a23d569a5c9966", // HEAD of main as of 2025-10-22
        )
        .await;

        println!("result: {:?}", result);

        assert!(result.is_ok(), "GitHub API request should succeed");
        let branch = result.unwrap();
        println!("Branch: {:?}", branch);
        assert!(branch.is_some(), "Should find branch for commit");
        assert_eq!(branch.as_deref(), Some("main"), "Should be main branch");
        println!("Branch: {:?}", branch);
    }

    #[test]
    fn test_parse_normalized_repo() {
        let repo = "github.com/alice/project";
        let parts: Vec<&str> = repo.split('/').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "github.com");
        assert_eq!(parts[1], "alice");
        assert_eq!(parts[2], "project");
    }
}
