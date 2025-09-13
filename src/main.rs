use chrono::Utc;
use clap::Parser;

use gitcircles_github::{
    cli::{
        Cli, Commands, ProjectCommands, display_project_details, display_projects,
        display_pull_requests, display_repository_status,
    },
    database::Database,
    github::GitHubClient,
    types::{
        GitCirclesError, Project, ProjectOwner, Repository, Result,
        generate_project_id, get_database_path, parse_repo,
    },
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
            project_id,
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

            // Validate project_id if provided
            if let Some(pid) = project_id
                && db.get_project(pid)?.is_none() {
                    return Err(GitCirclesError::DatabasePath(format!(
                        "Project '{}' not found",
                        pid
                    )));
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
                    project_id: project_id.clone(),
                });

            // Update project_id if provided
            if project_id.is_some() {
                repo_record.project_id = project_id.clone();
            }

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
        Commands::Status { project_id } => {
            let db = Database::new(&get_database_path()?)?;

            if let Some(pid) = project_id {
                // Show status for specific project
                let project = db.get_project(pid)?.ok_or_else(|| {
                    GitCirclesError::DatabasePath(format!(
                        "Project '{}' not found",
                        pid
                    ))
                })?;
                let owners = db.get_project_owners(pid)?;
                let repos = db.list_repositories_for_project(pid)?;
                display_project_details(&project, &owners, &repos);
            } else {
                // Show overall status
                let repos = db.list_repositories()?;
                let projects = db.list_projects()?;

                if !projects.is_empty() {
                    println!("📋 Projects:");
                    display_projects(&projects);
                    println!();
                }

                if !repos.is_empty() {
                    println!("📦 All Repositories:");
                    display_repository_status(&repos);
                } else if projects.is_empty() {
                    println!("No repositories or projects being tracked.");
                    println!(
                        "Use 'gitcircles-github collect --repo owner/repo' to start tracking repositories."
                    );
                    println!(
                        "Use 'gitcircles-github project create <name>' to create a project."
                    );
                }
            }
        }
        Commands::Init => {
            println!("Initializing GitCircles database...");
            let db_path = get_database_path()?;
            let _db = Database::new(&db_path)?;
            println!("✓ Database initialized at: {}", db_path);
        }
        Commands::Project(project_cmd) => {
            let db = Database::new(&get_database_path()?)?;

            match project_cmd {
                ProjectCommands::Create { name, description } => {
                    let project_id = generate_project_id(name);
                    let now = Utc::now();

                    let project = Project {
                        id: project_id.clone(),
                        name: name.clone(),
                        description: description.clone(),
                        created_at: now,
                        updated_at: now,
                    };

                    db.upsert_project(&project)?;
                    println!(
                        "✓ Created project '{}' with ID: {}",
                        name, project_id
                    );

                    if let Some(desc) = description {
                        println!("  Description: {}", desc);
                    }
                }
                ProjectCommands::List => {
                    let projects = db.list_projects()?;
                    display_projects(&projects);
                }
                ProjectCommands::Show { project_id } => {
                    let project = db.get_project(project_id)?.ok_or_else(|| {
                        GitCirclesError::DatabasePath(format!(
                            "Project '{}' not found",
                            project_id
                        ))
                    })?;
                    let owners = db.get_project_owners(project_id)?;
                    let repos = db.list_repositories_for_project(project_id)?;
                    display_project_details(&project, &owners, &repos);
                }
                ProjectCommands::Delete { project_id } => {
                    // Check if project exists
                    let project = db.get_project(project_id)?.ok_or_else(|| {
                        GitCirclesError::DatabasePath(format!(
                            "Project '{}' not found",
                            project_id
                        ))
                    })?;

                    // Check for linked repositories
                    let repos = db.list_repositories_for_project(project_id)?;
                    if !repos.is_empty() {
                        return Err(GitCirclesError::DatabasePath(format!(
                            "Cannot delete project '{}': {} repositories are still linked. Remove repositories first.",
                            project_id,
                            repos.len()
                        )));
                    }

                    // Remove all project owners first
                    let owners = db.get_project_owners(project_id)?;
                    for owner in &owners {
                        db.remove_project_owner(
                            project_id,
                            &owner.github_username,
                        )?;
                    }

                    // Delete the project
                    db.delete_project(project_id)?;
                    println!(
                        "✓ Deleted project '{}' ({})",
                        project.name, project_id
                    );
                }
                ProjectCommands::AddOwner {
                    project_id,
                    username,
                    role,
                } => {
                    // Validate project exists
                    let _project =
                        db.get_project(project_id)?.ok_or_else(|| {
                            GitCirclesError::DatabasePath(format!(
                                "Project '{}' not found",
                                project_id
                            ))
                        })?;

                    // Validate role
                    if !["owner", "admin", "member"].contains(&role.as_str()) {
                        return Err(GitCirclesError::DatabasePath(
                            "Invalid role. Must be one of: owner, admin, member"
                                .to_string(),
                        ));
                    }

                    let project_owner = ProjectOwner {
                        project_id: project_id.clone(),
                        github_username: username.clone(),
                        role: role.clone(),
                        added_at: Utc::now(),
                    };

                    db.add_project_owner(&project_owner)?;
                    println!(
                        "✓ Added {} as {} to project {}",
                        username, role, project_id
                    );
                }
                ProjectCommands::RemoveOwner {
                    project_id,
                    username,
                } => {
                    // Validate project exists
                    let _project =
                        db.get_project(project_id)?.ok_or_else(|| {
                            GitCirclesError::DatabasePath(format!(
                                "Project '{}' not found",
                                project_id
                            ))
                        })?;

                    db.remove_project_owner(project_id, username)?;
                    println!("✓ Removed {} from project {}", username, project_id);
                }
            }
        }
    }

    Ok(())
}
