use blake2::Blake2b;
use blake2::Digest;
use blake2::digest::{FixedOutput, Update, consts::U32};
use bs58;
use chrono::{DateTime, Utc};
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

/// Validates an Ergo mainnet Pay-to-Public-Key (P2PK) address.
///
/// # Examples
///
/// ```
/// // Valid mainnet P2PK address (starts with '9')
/// assert!(is_valid_p2pk_mainnet("9fRAWhdxEsTcdb8PhGNrZfwqa65zfkuYHAMmkQLcic1gdLSV5vA"));
///
/// // Invalid: testnet address (starts with '3')
/// assert!(!is_valid_p2pk_mainnet("3WvsT2Gm4EpsM9Pg18PdY6XyhNNMqXDsvJTbbf6ihLvAmSb7u5RN"));
///
/// // Invalid: corrupted checksum
/// assert!(!is_valid_p2pk_mainnet("9fRAWhdxEsTcdb8PhGNrZfwqa65zfkuYHAMmkQLcic1gdLSV5vB"));
/// ```
pub fn is_valid_p2pk_mainnet(addr: &str) -> bool {
    if !addr.starts_with('9') {
        return false;
    }

    let Ok(decoded) = bs58::decode(addr).into_vec() else {
        return false;
    };

    if decoded.len() != 38 {
        return false;
    }

    // SAFETY: we just checked len == 38
    let prefix = unsafe { *decoded.get_unchecked(0) };
    let content = unsafe { decoded.get_unchecked(1..34) };
    let checksum = unsafe { decoded.get_unchecked(34..38) };

    if prefix != 0x01 {
        return false;
    }

    if unsafe { *content.get_unchecked(0) } != 0x02
        && unsafe { *content.get_unchecked(0) } != 0x03
    {
        return false;
    }

    // Checksum prevents accidental typos from creating valid-looking addresses
    let mut hasher = Blake2b::<U32>::new();
    <Blake2b<U32> as Update>::update(&mut hasher, &decoded[0..34]);
    let hash = hasher.finalize_fixed();
    let computed_checksum = unsafe { &hash.get_unchecked(0..4) };

    checksum == *computed_checksum
}

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

        if is_valid_p2pk_mainnet(trimmed) {
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

    /// FIXME(Kivooeo): This is placeholder function I don't know why this needed
    /// As far as I see it's need for testing,
    /// So it should be fine me put a valid key here
    /// Before it was generating a sequence of 'A' with leading '9'
    /// To create pseudo correct address, now, when the check is real
    /// It's should give a valid address
    fn mk_valid(_: usize) -> String {
        String::from("9fRAWhdxEsTcdb8PhGNrZfwqa65zfkuYHAMmkQLcic1gdLSV5vA")
    }

    #[test]
    fn trims_whitespace() {
        let inner = mk_valid(60);
        let raw = format!("  {}\n", inner);
        let addr = WalletAddress::try_from(raw.as_str()).expect("should be valid");
        assert_eq!(addr.as_str(), inner);
    }

    #[test]
    fn valid() {
        assert!(is_valid_p2pk_mainnet(
            "9fRAWhdxEsTcdb8PhGNrZfwqa65zfkuYHAMmkQLcic1gdLSV5vA"
        ));
        assert!(is_valid_p2pk_mainnet(
            "9fZZEJVg7z29LARcVTffLKaxBW19dL1wiX34zSnE2rrWfMd2qcz"
        ));
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
        let s = &mk_valid(45)[0..45].to_string();
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
}
