use clap::{Parser, Subcommand};
use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};

use crate::types::{MergedPullRequest, Project, ProjectOwner, Repository};

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

        /// Project ID to associate this repository with (optional)
        #[arg(short, long)]
        project_id: Option<String>,
    },

    /// Show status of tracked repositories
    Status {
        /// Show status for a specific project only
        #[arg(short, long)]
        project_id: Option<String>,
    },

    /// Initialize local database
    Init,

    /// Manage projects
    #[command(subcommand)]
    Project(ProjectCommands),
}

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Create a new project
    Create {
        /// Project name
        name: String,

        /// Project description (optional)
        #[arg(short, long)]
        description: Option<String>,
    },

    /// List all projects
    List,

    /// Show detailed information about a project
    Show {
        /// Project ID
        project_id: String,
    },

    /// Delete a project
    Delete {
        /// Project ID
        project_id: String,
    },

    /// Add an owner to a project
    AddOwner {
        /// Project ID
        project_id: String,

        /// GitHub username
        username: String,

        /// Role (owner, admin, member)
        #[arg(short, long, default_value = "member")]
        role: String,
    },

    /// Remove an owner from a project
    RemoveOwner {
        /// Project ID
        project_id: String,

        /// GitHub username
        username: String,
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

pub fn display_projects(projects: &[Project]) {
    if projects.is_empty() {
        println!("No projects found.");
        println!(
            "Use 'gitcircles-github project create <name>' to create a project."
        );
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Project ID",
            "Name",
            "Description",
            "Created",
            "Updated",
        ]);

    for project in projects {
        table.add_row(vec![
            project.id.clone(),
            project.name.clone(),
            project
                .description
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            project.created_at.format("%Y-%m-%d").to_string(),
            project.updated_at.format("%Y-%m-%d").to_string(),
        ]);
    }

    println!("\n{}", table);
    println!("Total projects: {}", projects.len());
}

pub fn display_project_details(
    project: &Project,
    owners: &[ProjectOwner],
    repos: &[Repository],
) {
    println!("\n📋 Project: {}", project.name);
    println!("ID: {}", project.id);
    if let Some(desc) = &project.description {
        println!("Description: {}", desc);
    }
    println!(
        "Created: {}",
        project.created_at.format("%Y-%m-%d %H:%M UTC")
    );
    println!(
        "Updated: {}",
        project.updated_at.format("%Y-%m-%d %H:%M UTC")
    );

    println!("\n👥 Project Owners ({}):", owners.len());
    if !owners.is_empty() {
        let mut owners_table = Table::new();
        owners_table
            .load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec!["Username", "Role", "Added"]);

        for owner in owners {
            owners_table.add_row(vec![
                owner.github_username.clone(),
                owner.role.clone(),
                owner.added_at.format("%Y-%m-%d").to_string(),
            ]);
        }
        println!("{}", owners_table);
    } else {
        println!("  No owners added yet.");
    }

    println!("\n📦 Repositories ({}):", repos.len());
    if !repos.is_empty() {
        display_repository_status(repos);
    } else {
        println!("  No repositories tracked for this project yet.");
        println!(
            "  Use 'gitcircles-github collect --repo owner/repo --project-id {}' to add repositories.",
            project.id
        );
    }
}
