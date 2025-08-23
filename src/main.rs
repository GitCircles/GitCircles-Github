use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};
use indicatif::{ProgressBar, ProgressStyle};
use octocrab::{Octocrab, Page};
use serde::{Deserialize, Serialize};
use std::time::Duration;
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

pub struct GitHubClient {
    octocrab: Octocrab,
}

impl GitHubClient {
    pub fn new(token: &str) -> Result<Self> {
        let octocrab = Octocrab::builder()
            .personal_token(token.to_string())
            .build()?;

        Ok(Self { octocrab })
    }

    pub async fn fetch_merged_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        base_branch: &str,
        days_back: Option<u64>,
    ) -> Result<Vec<MergedPullRequest>> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.set_message("Fetching pull requests from GitHub...");
        pb.enable_steady_tick(Duration::from_millis(100));

        let mut merged_prs = Vec::new();
        let mut page = 1u32;
        let per_page = 100u8;

        // Calculate cutoff date if days_back is specified
        let cutoff_date =
            days_back.map(|days| Utc::now() - chrono::Duration::days(days as i64));

        loop {
            pb.set_message(format!("Fetching page {} from GitHub API...", page));

            let pulls_page: Page<octocrab::models::pulls::PullRequest> = self
                .octocrab
                .pulls(owner, repo)
                .list()
                .state(octocrab::params::State::Closed)
                .base(base_branch)
                .per_page(per_page)
                .page(page)
                .send()
                .await?;

            let pulls = pulls_page.items;
            if pulls.is_empty() {
                break;
            }

            pb.set_message(format!(
                "Processing {} PRs from page {}...",
                pulls.len(),
                page
            ));
            let pulls_len = pulls.len();

            for pr in pulls {
                // Only include merged PRs
                if let Some(merged_at) = pr.merged_at {
                    // Check if within date range if specified
                    if let Some(cutoff) = cutoff_date {
                        if merged_at < cutoff {
                            continue;
                        }
                    }

                    let merged_pr = MergedPullRequest {
                        number: pr.number,
                        title: pr.title.unwrap_or_else(|| "No title".to_string()),
                        author: pr
                            .user
                            .map(|u| u.login)
                            .unwrap_or_else(|| "unknown".to_string()),
                        merged_at,
                        base_branch: pr.base.ref_field,
                        merge_commit_sha: pr
                            .merge_commit_sha
                            .unwrap_or_else(|| "unknown".to_string()),
                        repository: format!("{}/{}", owner, repo),
                    };

                    merged_prs.push(merged_pr);
                }
            }

            // If this page wasn't full, we've reached the end
            if pulls_len < per_page as usize {
                break;
            }

            page += 1;
        }

        pb.finish_with_message(format!("✓ Found {} merged PRs", merged_prs.len()));
        Ok(merged_prs)
    }
}

fn display_pull_requests(prs: &[MergedPullRequest]) {
    if prs.is_empty() {
        println!("No merged pull requests found.");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "PR#",
            "Title",
            "Author",
            "Merged Date",
            "Base Branch",
            "Commit SHA",
        ]);

    for pr in prs {
        table.add_row(vec![
            pr.number.to_string(),
            if pr.title.len() > 50 {
                format!("{}...", &pr.title[..47])
            } else {
                pr.title.clone()
            },
            pr.author.clone(),
            pr.merged_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            pr.base_branch.clone(),
            pr.merge_commit_sha[..8].to_string(),
        ]);
    }

    println!("\n{}", table);
    println!("Total merged PRs: {}", prs.len());
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

            // Create GitHub client and fetch PRs
            let github_client = GitHubClient::new(&github_token)?;
            let merged_prs = github_client
                .fetch_merged_pull_requests(&owner, &repo_name, base_branch, *days)
                .await?;

            // Display results with comfy-table
            display_pull_requests(&merged_prs);
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
