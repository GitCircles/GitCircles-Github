use chrono::Utc;

use crate::types::{BaseBranchChange, MergedPullRequest, Project, ProjectOwner, Repository, Result};

pub struct Database {
    keyspace: fjall::Keyspace,
    repositories: fjall::PartitionHandle,
    pull_requests: fjall::PartitionHandle,
    base_branch_history: fjall::PartitionHandle,
    projects: fjall::PartitionHandle,
    project_owners: fjall::PartitionHandle,
}

impl Database {
    pub fn new(db_path: &str) -> Result<Self> {
        let keyspace = fjall::Config::new(db_path).open()?;

        let repositories = keyspace.open_partition(
            "repositories",
            fjall::PartitionCreateOptions::default(),
        )?;
        let pull_requests = keyspace.open_partition(
            "pull_requests",
            fjall::PartitionCreateOptions::default(),
        )?;
        let base_branch_history = keyspace.open_partition(
            "base_branch_history",
            fjall::PartitionCreateOptions::default(),
        )?;
        let projects = keyspace.open_partition(
            "projects",
            fjall::PartitionCreateOptions::default(),
        )?;
        let project_owners = keyspace.open_partition(
            "project_owners",
            fjall::PartitionCreateOptions::default(),
        )?;

        Ok(Self {
            keyspace,
            repositories,
            pull_requests,
            base_branch_history,
            projects,
            project_owners,
        })
    }

    pub fn upsert_repository(&self, repo: &Repository) -> Result<()> {
        let key = format!("repo:{}/{}", repo.owner, repo.name);
        let value = serde_json::to_vec(repo)?;
        self.repositories.insert(&key, &value)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn get_repository(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<Option<Repository>> {
        let key = format!("repo:{}/{}", owner, name);
        if let Some(value) = self.repositories.get(&key)? {
            let repo: Repository = serde_json::from_slice(&value)?;
            Ok(Some(repo))
        } else {
            Ok(None)
        }
    }

    pub fn list_repositories(&self) -> Result<Vec<Repository>> {
        let mut repos = Vec::new();
        for item in self.repositories.prefix("repo:".as_bytes()) {
            let (_, value) = item?;
            let repo: Repository = serde_json::from_slice(&value)?;
            repos.push(repo);
        }
        Ok(repos)
    }

    pub fn upsert_pull_request(&self, pr: &MergedPullRequest) -> Result<()> {
        let key = format!("pr:{}:{}", pr.repository, pr.number);
        let value = serde_json::to_vec(pr)?;
        self.pull_requests.insert(&key, &value)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn get_pull_requests(&self, repo: &str) -> Result<Vec<MergedPullRequest>> {
        let mut prs = Vec::new();
        let prefix = format!("pr:{}:", repo);
        for item in self.pull_requests.prefix(prefix.as_bytes()) {
            let (_, value) = item?;
            let pr: MergedPullRequest = serde_json::from_slice(&value)?;
            prs.push(pr);
        }
        Ok(prs)
    }

    pub fn pull_request_exists(&self, repo: &str, number: u64) -> Result<bool> {
        let key = format!("pr:{}:{}", repo, number);
        Ok(self.pull_requests.contains_key(&key)?)
    }

    pub fn record_base_branch_change(
        &self,
        repo: &str,
        old_branch: &str,
        new_branch: &str,
    ) -> Result<()> {
        let change = BaseBranchChange {
            repository: repo.to_string(),
            old_branch: old_branch.to_string(),
            new_branch: new_branch.to_string(),
            changed_at: Utc::now(),
        };

        let key = format!("base:{}:{}", repo, change.changed_at.timestamp());
        let value = serde_json::to_vec(&change)?;
        self.base_branch_history.insert(&key, &value)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn get_base_branch_history(
        &self,
        repo: &str,
    ) -> Result<Vec<BaseBranchChange>> {
        let mut changes = Vec::new();
        let prefix = format!("base:{}:", repo);
        for item in self.base_branch_history.prefix(prefix.as_bytes()) {
            let (_, value) = item?;
            let change: BaseBranchChange = serde_json::from_slice(&value)?;
            changes.push(change);
        }
        Ok(changes)
    }

    // Project methods
    pub fn upsert_project(&self, project: &Project) -> Result<()> {
        let key = format!("project:{}", project.id);
        let value = serde_json::to_vec(project)?;
        self.projects.insert(&key, &value)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn get_project(&self, project_id: &str) -> Result<Option<Project>> {
        let key = format!("project:{}", project_id);
        if let Some(value) = self.projects.get(&key)? {
            let project: Project = serde_json::from_slice(&value)?;
            Ok(Some(project))
        } else {
            Ok(None)
        }
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut projects = Vec::new();
        for item in self.projects.prefix("project:".as_bytes()) {
            let (_, value) = item?;
            let project: Project = serde_json::from_slice(&value)?;
            projects.push(project);
        }
        Ok(projects)
    }

    pub fn delete_project(&self, project_id: &str) -> Result<()> {
        let key = format!("project:{}", project_id);
        self.projects.remove(&key)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    // Project owner methods
    pub fn add_project_owner(&self, owner: &ProjectOwner) -> Result<()> {
        let key = format!("owner:{}:{}", owner.project_id, owner.github_username);
        let value = serde_json::to_vec(owner)?;
        self.project_owners.insert(&key, &value)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn get_project_owners(&self, project_id: &str) -> Result<Vec<ProjectOwner>> {
        let mut owners = Vec::new();
        let prefix = format!("owner:{}:", project_id);
        for item in self.project_owners.prefix(prefix.as_bytes()) {
            let (_, value) = item?;
            let owner: ProjectOwner = serde_json::from_slice(&value)?;
            owners.push(owner);
        }
        Ok(owners)
    }

    pub fn remove_project_owner(&self, project_id: &str, username: &str) -> Result<()> {
        let key = format!("owner:{}:{}", project_id, username);
        self.project_owners.remove(&key)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn get_projects_for_owner(&self, username: &str) -> Result<Vec<String>> {
        let mut project_ids = Vec::new();
        for item in self.project_owners.iter() {
            let (_key, value) = item?;
            let owner: ProjectOwner = serde_json::from_slice(&value)?;
            if owner.github_username == username {
                project_ids.push(owner.project_id);
            }
        }
        Ok(project_ids)
    }

    // Repository methods updated for project context
    pub fn list_repositories_for_project(&self, project_id: &str) -> Result<Vec<Repository>> {
        let mut repos = Vec::new();
        for item in self.repositories.prefix("repo:".as_bytes()) {
            let (_, value) = item?;
            let repo: Repository = serde_json::from_slice(&value)?;
            if repo.project_id.as_deref() == Some(project_id) {
                repos.push(repo);
            }
        }
        Ok(repos)
    }

    pub fn get_pull_requests_for_project(&self, project_id: &str) -> Result<Vec<MergedPullRequest>> {
        let mut all_prs = Vec::new();
        let repos = self.list_repositories_for_project(project_id)?;
        
        for repo in repos {
            let repo_str = format!("{}/{}", repo.owner, repo.name);
            let mut prs = self.get_pull_requests(&repo_str)?;
            all_prs.append(&mut prs);
        }
        
        // Sort by merged date, most recent first
        all_prs.sort_by(|a, b| b.merged_at.cmp(&a.merged_at));
        Ok(all_prs)
    }
}
