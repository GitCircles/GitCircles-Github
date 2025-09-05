use chrono::Utc;

use crate::types::{BaseBranchChange, MergedPullRequest, Repository, Result};

pub struct Database {
    keyspace: fjall::Keyspace,
    repositories: fjall::PartitionHandle,
    pull_requests: fjall::PartitionHandle,
    base_branch_history: fjall::PartitionHandle,
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

        Ok(Self {
            keyspace,
            repositories,
            pull_requests,
            base_branch_history,
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
}
