use clap::{Parser, Subcommand};
use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};

use crate::types::{MergedPullRequest, Repository};

#[derive(Parser)]
#[command(name = "gitcircles-github")]
#[command(about = "GitCircles GitHub adapter for collecting merged pull requests")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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

pub fn display_pull_requests(prs: &[MergedPullRequest]) {
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

pub fn display_repository_status(repos: &[Repository]) {
    if repos.is_empty() {
        println!("No repositories being tracked.");
        println!(
            "Use 'gitcircles-github collect --repo owner/repo' to start tracking."
        );
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Repository",
            "Base Branch",
            "Last Sync",
            "Total PRs",
            "First Tracked",
        ]);

    for repo in repos {
        table.add_row(vec![
            format!("{}/{}", repo.owner, repo.name),
            repo.current_base_branch.clone(),
            repo.last_sync
                .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| "Never".to_string()),
            repo.total_prs.to_string(),
            repo.first_sync.format("%Y-%m-%d").to_string(),
        ]);
    }

    println!("\n{}", table);
    println!("Total repositories tracked: {}", repos.len());
}
