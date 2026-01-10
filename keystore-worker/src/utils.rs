//! Utility functions

/// Normalize repository URL to consistent format (domain.com/owner/repo)
///
/// Examples:
/// - "https://github.com/alice/project" → "github.com/alice/project"
/// - "http://github.com/alice/project" → "github.com/alice/project"
/// - "ssh://git@github.com/alice/project" → "github.com/alice/project"
/// - "git@github.com:alice/project" → "github.com/alice/project"
/// - "github.com/alice/project.git" → "github.com/alice/project"
/// - "github.com/alice/project" → "github.com/alice/project"
pub fn normalize_repo_url(repo: &str) -> String {
    let mut repo = repo.trim().to_string();

    // Remove protocol (https://, http://, ssh://)
    if let Some(rest) = repo.strip_prefix("https://") {
        repo = rest.to_string();
    } else if let Some(rest) = repo.strip_prefix("http://") {
        repo = rest.to_string();
    } else if let Some(rest) = repo.strip_prefix("ssh://") {
        repo = rest.to_string();
    }

    // Handle git@ format (git@github.com:owner/repo or git@github.com/owner/repo)
    if let Some(rest) = repo.strip_prefix("git@") {
        repo = rest.replace(':', "/");
    }

    // Remove .git suffix if present
    if let Some(rest) = repo.strip_suffix(".git") {
        repo = rest.to_string();
    }

    repo
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
            normalize_repo_url("ssh://git@github.com/alice/project"),
            "github.com/alice/project"
        );
        assert_eq!(
            normalize_repo_url("git@github.com:alice/project"),
            "github.com/alice/project"
        );
        assert_eq!(
            normalize_repo_url("git@github.com/alice/project"),
            "github.com/alice/project"
        );
        assert_eq!(
            normalize_repo_url("github.com/alice/project.git"),
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
