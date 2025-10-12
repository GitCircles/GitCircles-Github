use chrono::Utc;
use clap::Parser;

use gitcircles_github::{
    cli::{
        Cli, Commands, WalletCommands, display_pull_requests,
        display_repository_status, display_user_wallet, display_wallet_history,
        display_wallet_logins,
    },
    database::Database,
    github::GitHubClient,
    types::{
        GitCirclesError, Repository, Result, WalletAddress, get_database_path,
        parse_repo,
    },
    wallet::WalletService,
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
        Commands::TestToken { token } => {
            let github_token = token.clone()
                .or_else(|| std::env::var("GITHUB_TOKEN").ok())
                .ok_or_else(|| GitCirclesError::Auth("GitHub token required. Use --token or set GITHUB_TOKEN environment variable".to_string()))?;

            println!("Testing GitHub token authentication...");
            let github_client = GitHubClient::new(&github_token)?;

            match github_client.test_token().await {
                Ok(username) => {
                    println!("✓ Token is valid!");
                    println!("✓ Authenticated as: {}", username);
                }
                Err(e) => {
                    eprintln!("✗ Token authentication failed!");
                    eprintln!("Error: {}", e);
                    eprintln!("\nTroubleshooting:");
                    eprintln!(
                        "1. Make sure your token starts with 'ghp_' or 'github_pat_'"
                    );
                    eprintln!(
                        "2. Generate a new token at: https://github.com/settings/tokens"
                    );
                    eprintln!("3. Required scopes: 'repo' or 'public_repo'");
                    return Err(e);
                }
            }
        }
        Commands::Wallet(wallet_cmd) => {
            let db = Database::new(&get_database_path()?)?;

            match wallet_cmd {
                WalletCommands::Sync { login, token } => {
                    // Get token from arg or environment
                    let github_token = token.clone()
                        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
                        .ok_or_else(|| GitCirclesError::Auth("GitHub token required. Use --token or set GITHUB_TOKEN environment variable".to_string()))?;

                    let github_client = GitHubClient::new(&github_token)?;

                    println!("Syncing wallet for GitHub user: {}", login);

                    let wallet_service = WalletService::new(&db, &github_client);
                    match wallet_service.sync_github_login(login).await? {
                        Some(result) => {
                            if result.changed {
                                if let Some(prev) = result.previous {
                                    println!(
                                        "✓ Wallet updated from {} to {}",
                                        prev, result.current
                                    );
                                } else {
                                    println!("✓ Wallet added: {}", result.current);
                                }
                            } else {
                                println!("✓ Wallet unchanged: {}", result.current);
                            }
                        }
                        None => println!("No wallet found for user '{}'", login),
                    }
                }
                WalletCommands::Show { login } => {
                    match db.get_user_wallet("github", login)? {
                        Some(wallet) => display_user_wallet(&wallet),
                        None => {
                            eprintln!("Error: No wallet found for user '{}'", login)
                        }
                    }
                }
                WalletCommands::History { login } => {
                    let history = db.get_wallet_history("github", login)?;
                    display_wallet_history(&history);
                }
                WalletCommands::Lookup { wallet } => {
                    let wallet_addr = WalletAddress::try_from(wallet.as_str())?;
                    let links = db.get_logins_for_wallet(&wallet_addr, "github")?;
                    let tuples: Vec<(String, String)> = links
                        .iter()
                        .map(|l| (l.platform.clone(), l.login.clone()))
                        .collect();
                    display_wallet_logins(&tuples);
                }
            }
        }
    }

    Ok(())
}
