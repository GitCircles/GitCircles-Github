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
    GitHubProfileRepo {
        login: String,
        branch: String,
        commit: String,
    },
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
pub struct Repository {
    pub owner: String,
    pub name: String,
    pub current_base_branch: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub total_prs: u64,
    pub first_sync: DateTime<Utc>,
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
