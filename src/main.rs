use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
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
}

type Result<T> = std::result::Result<T, GitCirclesError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub owner: String,
    pub name: String,
    pub current_base_branch: String,
    pub last_sync: Option<DateTime<Utc>>,
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

fn parse_repo(repo_str: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = repo_str.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(GitCirclesError::InvalidRepo(repo_str.to_string()));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[derive(Parser)]
#[command(name = "gitcircles-github")]
#[command(about = "GitCircles GitHub adapter for collecting merged pull requests")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Collect merged pull requests from a repository
    Collect {
        /// Repository in format "owner/repo"
        #[arg(short, long)]
        repo: String,

        /// GitHub personal access token
        #[arg(short, long)]
        token: Option<String>,

        /// Target base branch (default: main)
        #[arg(short, long, default_value = "main")]
        base_branch: String,

        /// Number of days to look back (optional)
        #[arg(short, long)]
        days: Option<u64>,
    },

    /// Show status of tracked repositories
    Status,

    /// Initialize local database
    Init,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Collect {
            repo,
            token,
            base_branch,
            days,
        } => {
            let (owner, repo_name) = parse_repo(repo)?;

            // Get token from arg or environment
            let github_token = token.clone()
                .or_else(|| std::env::var("GITHUB_TOKEN").ok())
                .ok_or_else(|| GitCirclesError::Auth("GitHub token required. Use --token or set GITHUB_TOKEN environment variable".to_string()))?;

            println!(
                "Collecting merged PRs from {}/{} (base: {})",
                owner, repo_name, base_branch
            );
            if let Some(days) = days {
                println!("Looking back {} days", days);
            }
            println!(
                "Using token: {}...",
                &github_token[..8.min(github_token.len())]
            );
            // TODO: Implement collection logic
        }
        Commands::Status => {
            println!("Repository tracking status");
            // TODO: Show tracked repos and last sync times
        }
        Commands::Init => {
            println!("Initializing database");
            // TODO: Initialize fjall database
        }
    }

    Ok(())
}
