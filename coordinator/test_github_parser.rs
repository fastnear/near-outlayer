// Standalone test for GitHub URL parsing
use regex::Regex;
use anyhow::{bail, Result};

/// Parse and normalize GitHub repository URL to standard format.
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

fn main() {
    let test_cases = vec![
        ("https://github.com/alice/project.git", "github.com/alice/project"),
        ("https://github.com/alice/project", "github.com/alice/project"),
        ("git@github.com:alice/project.git", "github.com/alice/project"),
        ("git@github.com:alice/project", "github.com/alice/project"),
        ("github.com/alice/project", "github.com/alice/project"),
        ("alice/project", "github.com/alice/project"),
        ("https://github.com/fastnear/near-offshore.git", "github.com/fastnear/near-offshore"),
        ("fastnear/near-offshore", "github.com/fastnear/near-offshore"),
        ("https://gitlab.com/alice/project.git", "gitlab.com/alice/project"),
    ];

    println!("Testing GitHub URL parser...\n");
    
    let mut passed = 0;
    let mut failed = 0;
    
    for (input, expected) in test_cases {
        match parse_github_repo(input) {
            Ok(result) => {
                if result == expected {
                    println!("✅ PASS: '{}' → '{}'", input, result);
                    passed += 1;
                } else {
                    println!("❌ FAIL: '{}' → '{}' (expected '{}')", input, result, expected);
                    failed += 1;
                }
            }
            Err(e) => {
                println!("❌ ERROR: '{}' → {}", input, e);
                failed += 1;
            }
        }
    }
    
    // Test error cases
    println!("\nTesting error cases...");
    let error_cases = vec!["", "not-a-repo", "https://github.com/"];
    for input in error_cases {
        match parse_github_repo(input) {
            Ok(result) => {
                println!("❌ UNEXPECTED SUCCESS: '{}' → '{}'", input, result);
                failed += 1;
            }
            Err(_) => {
                println!("✅ PASS: '{}' → error (as expected)", input);
                passed += 1;
            }
        }
    }
    
    println!("\n{} passed, {} failed", passed, failed);
    if failed > 0 {
        std::process::exit(1);
    }
}
