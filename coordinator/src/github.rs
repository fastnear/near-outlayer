use anyhow::{bail, Result};
use regex::Regex;

/// Parse and normalize GitHub repository URL to standard format.
///
/// Supports various input formats:
/// - `https://github.com/owner/repo.git`
/// - `https://github.com/owner/repo`
/// - `git@github.com:owner/repo.git`
/// - `github.com/owner/repo`
/// - `owner/repo` (assumes github.com)
///
/// Returns normalized format: `"github.com/owner/repo"`
///
/// # Examples
///
/// ```
/// use coordinator::github::parse_github_repo;
///
/// assert_eq!(
///     parse_github_repo("https://github.com/alice/project.git").unwrap(),
///     "github.com/alice/project"
/// );
/// assert_eq!(
///     parse_github_repo("alice/project").unwrap(),
///     "github.com/alice/project"
/// );
/// ```
pub fn parse_github_repo(input: &str) -> Result<String> {
    let input = input.trim();

    if input.is_empty() {
        bail!("Repository URL cannot be empty");
    }

    // Pattern 1: SSH format - git@github.com:owner/repo.git
    let ssh_re = Regex::new(r"^git@([^:]+):([^/]+)/([^/]+?)(?:\.git)?$").unwrap();
    if let Some(captures) = ssh_re.captures(input) {
        let host = captures.get(1).unwrap().as_str();
        let owner = captures.get(2).unwrap().as_str();
        let repo = captures.get(3).unwrap().as_str();
        return Ok(format!("{}/{}/{}", host, owner, repo));
    }

    // Pattern 2: HTTPS format - https://github.com/owner/repo[.git]
    let https_re = Regex::new(r"^https?://([^/]+)/([^/]+)/([^/]+?)(?:\.git)?$").unwrap();
    if let Some(captures) = https_re.captures(input) {
        let host = captures.get(1).unwrap().as_str();
        let owner = captures.get(2).unwrap().as_str();
        let repo = captures.get(3).unwrap().as_str();
        return Ok(format!("{}/{}/{}", host, owner, repo));
    }

    // Pattern 3: host/owner/repo format - github.com/owner/repo
    let host_re = Regex::new(r"^([^/]+)/([^/]+)/([^/]+?)(?:\.git)?$").unwrap();
    if let Some(captures) = host_re.captures(input) {
        let host = captures.get(1).unwrap().as_str();
        let owner = captures.get(2).unwrap().as_str();
        let repo = captures.get(3).unwrap().as_str();
        return Ok(format!("{}/{}/{}", host, owner, repo));
    }

    // Pattern 4: owner/repo format (assume github.com)
    let short_re = Regex::new(r"^([^/]+)/([^/]+?)(?:\.git)?$").unwrap();
    if let Some(captures) = short_re.captures(input) {
        let owner = captures.get(1).unwrap().as_str();
        let repo = captures.get(2).unwrap().as_str();
        return Ok(format!("github.com/{}/{}", owner, repo));
    }

    bail!("Invalid repository format: {}", input);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_https_with_git() {
        assert_eq!(
            parse_github_repo("https://github.com/alice/project.git").unwrap(),
            "github.com/alice/project"
        );
    }

    #[test]
    fn test_parse_https_without_git() {
        assert_eq!(
            parse_github_repo("https://github.com/alice/project").unwrap(),
            "github.com/alice/project"
        );
    }

    #[test]
    fn test_parse_ssh() {
        assert_eq!(
            parse_github_repo("git@github.com:alice/project.git").unwrap(),
            "github.com/alice/project"
        );
    }

    #[test]
    fn test_parse_ssh_without_git() {
        assert_eq!(
            parse_github_repo("git@github.com:alice/project").unwrap(),
            "github.com/alice/project"
        );
    }

    #[test]
    fn test_parse_host_owner_repo() {
        assert_eq!(
            parse_github_repo("github.com/alice/project").unwrap(),
            "github.com/alice/project"
        );
    }

    #[test]
    fn test_parse_host_owner_repo_with_git() {
        assert_eq!(
            parse_github_repo("github.com/alice/project.git").unwrap(),
            "github.com/alice/project"
        );
    }

    #[test]
    fn test_parse_short_format() {
        assert_eq!(
            parse_github_repo("alice/project").unwrap(),
            "github.com/alice/project"
        );
    }

    #[test]
    fn test_parse_short_format_with_git() {
        assert_eq!(
            parse_github_repo("alice/project.git").unwrap(),
            "github.com/alice/project"
        );
    }

    #[test]
    fn test_parse_gitlab() {
        assert_eq!(
            parse_github_repo("https://gitlab.com/alice/project.git").unwrap(),
            "gitlab.com/alice/project"
        );
    }

    #[test]
    fn test_parse_custom_host() {
        assert_eq!(
            parse_github_repo("git.example.com/alice/project").unwrap(),
            "git.example.com/alice/project"
        );
    }

    #[test]
    fn test_parse_empty_string() {
        assert!(parse_github_repo("").is_err());
    }

    #[test]
    fn test_parse_whitespace() {
        assert_eq!(
            parse_github_repo("  alice/project  ").unwrap(),
            "github.com/alice/project"
        );
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!(parse_github_repo("not-a-valid-repo").is_err());
        assert!(parse_github_repo("https://github.com/").is_err());
        assert!(parse_github_repo("github.com").is_err());
    }

    #[test]
    fn test_parse_fastnear_example() {
        assert_eq!(
            parse_github_repo("https://github.com/fastnear/near-offshore.git").unwrap(),
            "github.com/fastnear/near-offshore"
        );
        assert_eq!(
            parse_github_repo("git@github.com:fastnear/near-offshore.git").unwrap(),
            "github.com/fastnear/near-offshore"
        );
        assert_eq!(
            parse_github_repo("github.com/fastnear/near-offshore").unwrap(),
            "github.com/fastnear/near-offshore"
        );
        assert_eq!(
            parse_github_repo("fastnear/near-offshore").unwrap(),
            "github.com/fastnear/near-offshore"
        );
    }
}
