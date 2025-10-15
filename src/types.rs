use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitCirclesError {
    #[error("GitHub API error: {0}")]
    GitHub(#[from] octocrab::Error),

    #[error("Database error: {0}")]
    Database(#[from] fjall::Error),

    #[error("Invalid repository format: {0}. Expected 'owner/repo'")]
    InvalidRepo(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Database path error: {0}")]
    DatabasePath(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Wallet not found for {0}")]
    WalletNotFound(String),

    #[error("Invalid wallet address '{0}': {1}")]
    WalletInvalidFormat(String, String),

    #[error(
        "Repository {0} is not accessible. Profile repositories must be public."
    )]
    RepoNotAccessible(String),

    #[error(
        "Repository {0} exists but appears to be empty. Please create at least one commit with P2PK.pub file."
    )]
    RepoEmpty(String),
}

pub type Result<T> = std::result::Result<T, GitCirclesError>;

pub static WALLET_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^9[1-9A-HJ-NP-Za-km-z]{50,}$").expect("wallet regex must compile")
});

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WalletAddress(String);

impl WalletAddress {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WalletAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Deref for WalletAddress {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl TryFrom<&str> for WalletAddress {
    type Error = GitCirclesError;

    fn try_from(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if WALLET_REGEX.is_match(trimmed) {
            Ok(Self(trimmed.to_string()))
        } else {
            Err(GitCirclesError::WalletInvalidFormat(
                trimmed.to_string(),
                "expected Ergo P2PK".into(),
            ))
        }
    }
}

impl TryFrom<String> for WalletAddress {
    type Error = GitCirclesError;

    fn try_from(raw: String) -> Result<Self> {
        WalletAddress::try_from(raw.as_str())
    }
}

impl From<WalletAddress> for String {
    fn from(value: WalletAddress) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WalletSource {
    GitHubProfileRepo { login: String, branch: String },
}

#[derive(Debug, Clone)]
pub struct WalletFetchOutcome {
    pub address: WalletAddress,
    pub branch: String,
}

#[derive(Debug, Clone)]
pub struct WalletSyncResult {
    pub current: WalletAddress,
    pub previous: Option<WalletAddress>,
    pub changed: bool,
    pub source: WalletSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserWallet {
    pub login: String,
    pub platform: String,
    pub address: WalletAddress,
    pub source: WalletSource,
    pub synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletHistoryEntry {
    pub login: String,
    pub platform: String,
    pub address: WalletAddress,
    pub source: WalletSource,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletLoginLink {
    pub wallet: WalletAddress,
    pub platform: String,
    pub login: String,
    pub linked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOwner {
    pub project_id: String,
    pub github_username: String,
    pub role: String, // "owner", "admin", "member"
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub owner: String,
    pub name: String,
    pub current_base_branch: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub total_prs: u64,
    pub first_sync: DateTime<Utc>,
    pub project_id: Option<String>, // Link to project
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedPullRequest {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub merged_at: DateTime<Utc>,
    pub base_branch: String,
    pub merge_commit_sha: String,
    pub repository: String, // "owner/repo" format (TODO: separate type)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseBranchChange {
    pub repository: String,
    pub old_branch: String,
    pub new_branch: String,
    pub changed_at: DateTime<Utc>,
}

pub fn parse_repo(repo_str: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = repo_str.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(GitCirclesError::InvalidRepo(repo_str.to_string()));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

pub fn generate_project_id(name: &str) -> String {
    let timestamp = Utc::now().timestamp();
    let name_slug = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    format!("{}_{}", name_slug, timestamp)
}

pub fn get_database_path() -> Result<String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| {
            GitCirclesError::DatabasePath(
                "Cannot determine home directory".to_string(),
            )
        })?;

    let db_dir = format!("{}/.gitcircles", home);
    std::fs::create_dir_all(&db_dir).map_err(|e| {
        GitCirclesError::DatabasePath(format!(
            "Cannot create database directory: {}",
            e
        ))
    })?;
    Ok(format!("{}/db", db_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_valid(len: usize) -> String {
        // Build a valid Base58-ish string starting with '9'
        let mut s = String::from("9");
        // Allowed characters include 'A'; fill to requested length
        s.push_str(&"A".repeat(len - 1));
        s
    }

    #[test]
    fn valid_wallet_min_length() {
        let s = mk_valid(51);
        let addr = WalletAddress::try_from(s.as_str()).expect("should be valid");
        assert_eq!(addr.as_str(), s);
    }

    #[test]
    fn trims_whitespace() {
        let inner = mk_valid(60);
        let raw = format!("  {}\n", inner);
        let addr = WalletAddress::try_from(raw.as_str()).expect("should be valid");
        assert_eq!(addr.as_str(), inner);
    }

    #[test]
    fn invalid_prefix() {
        let mut s = mk_valid(51);
        s.replace_range(0..1, "8");
        let err = WalletAddress::try_from(s.as_str()).unwrap_err();
        match err {
            GitCirclesError::WalletInvalidFormat(_, _) => {}
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn invalid_too_short() {
        let s = mk_valid(45);
        let err = WalletAddress::try_from(s.as_str()).unwrap_err();
        match err {
            GitCirclesError::WalletInvalidFormat(_, _) => {}
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn invalid_chars() {
        let s = format!("9{}", "*".repeat(60));
        let err = WalletAddress::try_from(s.as_str()).unwrap_err();
        match err {
            GitCirclesError::WalletInvalidFormat(_, _) => {}
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn try_from_string() {
        let s = mk_valid(55);
        let addr = WalletAddress::try_from(s.clone()).expect("valid");
        assert_eq!(String::from(addr), s);
    }
}
