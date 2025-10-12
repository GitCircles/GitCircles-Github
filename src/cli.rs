use clap::{Parser, Subcommand};
use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};

use crate::types::{MergedPullRequest, Repository, UserWallet, WalletHistoryEntry};

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

    /// Test GitHub token authentication
    TestToken {
        /// GitHub personal access token
        #[arg(short, long)]
        token: Option<String>,
    },

    /// Wallet management commands
    #[command(subcommand)]
    Wallet(WalletCommands),
}

#[derive(Subcommand)]
pub enum WalletCommands {
    /// Fetch and sync wallet address for a GitHub user
    ///
    /// Reads P2PK.pub from <login>/gitcircles-payment-address repository.
    /// Token can be provided via --token or GITHUB_TOKEN environment variable.
    Sync {
        /// GitHub username
        login: String,

        /// GitHub personal access token
        #[arg(short, long)]
        token: Option<String>,
    },

    /// Show current wallet address for a GitHub user
    Show {
        /// GitHub username
        login: String,
    },

    /// Show wallet address history for a GitHub user
    History {
        /// GitHub username
        login: String,
    },

    /// Find all GitHub logins associated with a wallet address
    Lookup {
        /// Wallet address
        wallet: String,
    },
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

pub fn display_user_wallet(wallet: &UserWallet) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);

    table.add_row(vec!["Platform", &wallet.platform]);
    table.add_row(vec!["Login", &wallet.login]);
    table.add_row(vec!["Wallet Address", wallet.address.as_str()]);
    table.add_row(vec![
        "Last Synced",
        &wallet.synced_at.format("%Y-%m-%d %H:%M UTC").to_string(),
    ]);
    table.add_row(vec!["Source", &format!("{:?}", wallet.source)]);

    println!("\n{}", table);
}

pub fn display_wallet_history(history: &[WalletHistoryEntry]) {
    if history.is_empty() {
        println!("No wallet history found.");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Timestamp", "Wallet Address", "Source"]);

    for entry in history {
        table.add_row(vec![
            entry.recorded_at.format("%Y-%m-%d %H:%M UTC").to_string(),
            entry.address.as_str().to_string(),
            format!("{:?}", entry.source),
        ]);
    }

    println!("\n{}", table);
    println!("Total history entries: {}", history.len());
}

pub fn display_wallet_logins(logins: &[(String, String)]) {
    if logins.is_empty() {
        println!("No logins found for this wallet address.");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["Platform", "Login"]);

    for (platform, login) in logins {
        table.add_row(vec![platform.clone(), login.clone()]);
    }

    println!("\n{}", table);
    println!("Total logins: {}", logins.len());
}
