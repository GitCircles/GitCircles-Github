use chrono::Utc;
use clap::Parser;

use gitcircles_github::{
    cli::{Cli, Commands, display_pull_requests, display_repository_status},
    database::Database,
    github::GitHubClient,
    types::{GitCirclesError, Repository, Result, get_database_path, parse_repo},
};

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
            let db = Database::new(&get_database_path()?)?;
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

            // Check/update repository tracking
            let mut repo_record = db
                .get_repository(&owner, &repo_name)?
                .unwrap_or_else(|| Repository {
                    owner: owner.clone(),
                    name: repo_name.clone(),
                    current_base_branch: base_branch.clone(),
                    last_sync: None,
                    total_prs: 0,
                    first_sync: Utc::now(),
                });

            // Detect base branch changes
            if repo_record.current_base_branch != *base_branch {
                db.record_base_branch_change(
                    &format!("{}/{}", owner, repo_name),
                    &repo_record.current_base_branch,
                    base_branch,
                )?;
                println!(
                    "📝 Base branch changed from '{}' to '{}'",
                    repo_record.current_base_branch, base_branch
                );
                repo_record.current_base_branch = base_branch.clone();
            }

            // Create GitHub client and fetch PRs
            let github_client = GitHubClient::new(&github_token)?;
            let fetched_prs = github_client
                .fetch_merged_pull_requests(&owner, &repo_name, base_branch, *days)
                .await?;

            // Filter out already-stored PRs (deduplication)
            let mut new_prs = Vec::new();
            for pr in fetched_prs {
                if !db.pull_request_exists(&pr.repository, pr.number)? {
                    db.upsert_pull_request(&pr)?;
                    new_prs.push(pr);
                }
            }

            // Update repository metadata
            repo_record.last_sync = Some(Utc::now());
            repo_record.total_prs += new_prs.len() as u64;
            db.upsert_repository(&repo_record)?;

            // Display results
            if new_prs.is_empty() {
                println!(
                    "No new merged PRs found. {} total PRs tracked.",
                    repo_record.total_prs
                );
            } else {
                display_pull_requests(&new_prs);
                println!(
                    "✓ Added {} new PRs. {} total PRs tracked.",
                    new_prs.len(),
                    repo_record.total_prs
                );
            }
        }
        Commands::Status => {
            let db = Database::new(&get_database_path()?)?;
            let repos = db.list_repositories()?;
            display_repository_status(&repos);
        }
        Commands::Init => {
            println!("Initializing GitCircles database...");
            let db_path = get_database_path()?;
            let _db = Database::new(&db_path)?;
            println!("✓ Database initialized at: {}", db_path);
        }
    }

    Ok(())
}
