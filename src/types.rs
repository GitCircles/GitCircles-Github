use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
}

pub type Result<T> = std::result::Result<T, GitCirclesError>;

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
