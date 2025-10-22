//! Utility functions

/// Normalize repository URL to consistent format (domain.com/owner/repo)
///
/// Examples:
/// - "https://github.com/alice/project" → "github.com/alice/project"
/// - "http://github.com/alice/project" → "github.com/alice/project"
/// - "github.com/alice/project" → "github.com/alice/project"
/// - "gitlab.com/alice/project" → "gitlab.com/alice/project"
pub fn normalize_repo_url(repo: &str) -> String {
    let repo = repo.trim();

    // Remove protocol (https:// or http://)
    let repo = repo
        .strip_prefix("https://")
        .or_else(|| repo.strip_prefix("http://"))
        .unwrap_or(repo);

    repo.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_repo_url() {
        assert_eq!(
            normalize_repo_url("https://github.com/alice/project"),
            "github.com/alice/project"
        );
        assert_eq!(
            normalize_repo_url("http://github.com/alice/project"),
            "github.com/alice/project"
        );
        assert_eq!(
            normalize_repo_url("github.com/alice/project"),
            "github.com/alice/project"
        );
        assert_eq!(
            normalize_repo_url("gitlab.com/alice/project"),
            "gitlab.com/alice/project"
        );
    }
}
