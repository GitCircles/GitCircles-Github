use chrono::Utc;

use crate::types::{
    BaseBranchChange, MergedPullRequest, Project, ProjectOwner, Repository, Result,
    UserWallet, WalletAddress, WalletHistoryEntry, WalletLoginLink,
};

pub struct Database {
    pub keyspace: fjall::Keyspace,
    repositories: fjall::PartitionHandle,
    pull_requests: fjall::PartitionHandle,
    base_branch_history: fjall::PartitionHandle,
    user_wallets: fjall::PartitionHandle,
    user_wallet_history: fjall::PartitionHandle,
    wallet_index: fjall::PartitionHandle,
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
        let user_wallets = keyspace.open_partition(
            "user_wallets",
            fjall::PartitionCreateOptions::default(),
        )?;
        let user_wallet_history = keyspace.open_partition(
            "user_wallet_history",
            fjall::PartitionCreateOptions::default(),
        )?;
        let wallet_index = keyspace.open_partition(
            "wallet_index",
            fjall::PartitionCreateOptions::default(),
        )?;
        let projects = keyspace
            .open_partition("projects", fjall::PartitionCreateOptions::default())?;
        let project_owners = keyspace.open_partition(
            "project_owners",
            fjall::PartitionCreateOptions::default(),
        )?;

        Ok(Self {
            keyspace,
            repositories,
            pull_requests,
            base_branch_history,
            user_wallets,
            user_wallet_history,
            wallet_index,
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
        if let Some(value) = self.repositories.get(&key)?
            && let Ok(repo) = serde_json::from_slice(&value)
        {
            Ok(Some(repo))
        } else {
            Ok(None)
        }
    }

    pub fn list_repositories(&self) -> Result<Vec<Repository>> {
        self.repositories
            .prefix("repo:".as_bytes())
            .map(|item| {
                let (_, value) = item?;
                let repo: Repository = serde_json::from_slice(&value)?;
                Ok(repo)
            })
            .collect()
    }

    pub fn upsert_pull_request(&self, pr: &MergedPullRequest) -> Result<()> {
        let key = format!("pr:{}:{}", pr.repository, pr.number);
        let value = serde_json::to_vec(pr)?;
        self.pull_requests.insert(&key, &value)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn get_pull_requests(&self, repo: &str) -> Result<Vec<MergedPullRequest>> {
        let prefix = format!("pr:{}:", repo);
        self.pull_requests
            .prefix(prefix.as_bytes())
            .map(|item| {
                let (_, value) = item?;
                let pr: MergedPullRequest = serde_json::from_slice(&value)?;
                Ok(pr)
            })
            .collect()
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
        let prefix = format!("base:{}:", repo);
        self.base_branch_history
            .prefix(prefix.as_bytes())
            .map(|item| {
                let (_, value) = item?;
                let change: BaseBranchChange = serde_json::from_slice(&value)?;
                Ok(change)
            })
            .collect()
    }

    // Wallet methods
    pub fn upsert_user_wallet(&self, wallet: &UserWallet) -> Result<()> {
        let key = format!("login:{}:{}", wallet.platform, wallet.login);
        let value = serde_json::to_vec(wallet)?;
        self.user_wallets.insert(&key, &value)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn get_user_wallet(
        &self,
        platform: &str,
        login: &str,
    ) -> Result<Option<UserWallet>> {
        let key = format!("login:{}:{}", platform, login);
        if let Some(value) = self.user_wallets.get(&key)?
            && let Ok(wallet) = serde_json::from_slice(&value)
        {
            Ok(Some(wallet))
        } else {
            Ok(None)
        }
    }

    pub fn append_wallet_history(&self, entry: &WalletHistoryEntry) -> Result<()> {
        let key = format!(
            "history:{}:{}:{}",
            entry.platform,
            entry.login,
            entry.recorded_at.timestamp()
        );
        let value = serde_json::to_vec(entry)?;
        self.user_wallet_history.insert(&key, &value)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn get_wallet_history(
        &self,
        platform: &str,
        login: &str,
    ) -> Result<Vec<WalletHistoryEntry>> {
        let prefix = format!("history:{}:{}:", platform, login);
        self.user_wallet_history
            .prefix(prefix.as_bytes())
            .map(|item| {
                let (_, value) = item?;
                let entry: WalletHistoryEntry = serde_json::from_slice(&value)?;
                Ok(entry)
            })
            .collect()
    }

    pub fn get_logins_for_wallet(
        &self,
        address: &WalletAddress,
        platform: &str,
    ) -> Result<Vec<WalletLoginLink>> {
        let prefix = format!("wallet:{}:{}:", address, platform);
        self.wallet_index
            .prefix(prefix.as_bytes())
            .map(|item| {
                let (_, value) = item?;
                let link: WalletLoginLink = serde_json::from_slice(&value)?;
                Ok(link)
            })
            .collect()
    }

    pub fn replace_wallet_link(&self, link: &WalletLoginLink) -> Result<()> {
        let key =
            format!("wallet:{}:{}:{}", link.wallet, link.platform, link.login);
        let value = serde_json::to_vec(link)?;
        self.wallet_index.insert(&key, &value)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    // Batch-aware methods for atomic wallet operations
    pub fn upsert_user_wallet_batch(
        &self,
        batch: &mut fjall::Batch,
        wallet: &UserWallet,
    ) -> Result<()> {
        let key = format!("login:{}:{}", wallet.platform, wallet.login);
        let value = serde_json::to_vec(wallet)?;
        batch.insert(&self.user_wallets, key, value);
        Ok(())
    }

    pub fn append_wallet_history_batch(
        &self,
        batch: &mut fjall::Batch,
        entry: &WalletHistoryEntry,
    ) -> Result<()> {
        let key = format!(
            "history:{}:{}:{}",
            entry.platform,
            entry.login,
            entry.recorded_at.timestamp()
        );
        let value = serde_json::to_vec(entry)?;
        batch.insert(&self.user_wallet_history, key, value);
        Ok(())
    }

    pub fn replace_wallet_link_batch(
        &self,
        batch: &mut fjall::Batch,
        link: &WalletLoginLink,
    ) -> Result<()> {
        let key =
            format!("wallet:{}:{}:{}", link.wallet, link.platform, link.login);
        let value = serde_json::to_vec(link)?;
        batch.insert(&self.wallet_index, key, value);
        Ok(())
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
        if let Some(value) = self.projects.get(&key)?
            && let Ok(project) = serde_json::from_slice(&value)
        {
            Ok(Some(project))
        } else {
            Ok(None)
        }
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        self.projects
            .prefix("project:".as_bytes())
            .map(|item| {
                let (_, value) = item?;
                let project: Project = serde_json::from_slice(&value)?;
                Ok(project)
            })
            .collect()
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

    pub fn get_project_owners(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectOwner>> {
        let prefix = format!("owner:{}:", project_id);
        self.project_owners
            .prefix(prefix.as_bytes())
            .map(|item| {
                let (_, value) = item?;
                let owner: ProjectOwner = serde_json::from_slice(&value)?;
                Ok(owner)
            })
            .collect()
    }

    pub fn remove_project_owner(
        &self,
        project_id: &str,
        username: &str,
    ) -> Result<()> {
        let key = format!("owner:{}:{}", project_id, username);
        self.project_owners.remove(&key)?;
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    pub fn get_projects_for_owner(&self, username: &str) -> Result<Vec<String>> {
        self.project_owners
            .iter()
            .map(|item| {
                let (_, value) = item?;
                let owner: ProjectOwner = serde_json::from_slice(&value)?;
                Ok(owner)
            })
            .filter_map(|result| match result {
                Ok(owner) if owner.github_username == username => {
                    Some(Ok(owner.project_id))
                }
                Ok(_) => None,
                Err(e) => Some(Err(e)),
            })
            .collect()
    }

    // Repository methods updated for project context
    pub fn list_repositories_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<Repository>> {
        self.repositories
            .prefix("repo:".as_bytes())
            .map(|item| {
                let (_, value) = item?;
                let repo: Repository = serde_json::from_slice(&value)?;
                if repo.project_id.as_deref() != Some(project_id) {
                    return Ok(None);
                }
                Ok(Some(repo))
            })
            .filter_map(Result::transpose)
            .collect()
    }

    pub fn get_pull_requests_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<MergedPullRequest>> {
        let repos = self.list_repositories_for_project(project_id)?;

        let mut all_prs = repos
            .into_iter()
            .flat_map(|repo| {
                let repo_str = format!("{}/{}", repo.owner, repo.name);
                self.get_pull_requests(&repo_str).unwrap_or_default()
            })
            .collect::<Vec<_>>();

        all_prs.sort_by(|a, b| b.merged_at.cmp(&a.merged_at));
        Ok(all_prs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WalletSource;
    use chrono::TimeZone;
    use tempfile::tempdir;

    /// FIXME(Kivooeo): This is placeholder function I don't know why this needed
    /// As far as I see it's need for testing,
    /// So it should be fine me put a valid key here
    /// Before it was generating a sequence of 'A' with leading '9'
    /// To create pseudo correct address, now, when the check is real
    /// It's should give a valid address
    fn addr() -> WalletAddress {
        let base =
            "9hUzb5RvSgDqJdtyCN9Ke496Yy63mpcUJKbRq4swzQ5EQKgygKT".to_string();
        WalletAddress::try_from(base.as_str()).unwrap()
    }

    #[test]
    fn user_wallet_roundtrip() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().to_str().unwrap()).unwrap();

        let uw = UserWallet {
            login: "alice".into(),
            platform: "github".into(),
            address: addr(),
            source: WalletSource::GitHubProfileRepo {
                login: "alice".into(),
                branch: "main".into(),
            },
            synced_at: Utc::now(),
        };
        db.upsert_user_wallet(&uw).unwrap();
        let fetched = db.get_user_wallet("github", "alice").unwrap().unwrap();
        assert_eq!(fetched.login, "alice");
        assert_eq!(fetched.address, uw.address);
    }

    #[test]
    fn wallet_history_ordering() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().to_str().unwrap()).unwrap();

        let login = "bob";
        let platform = "github";
        let addr1 = addr();
        let addr2 = addr();

        let e1 = WalletHistoryEntry {
            login: login.into(),
            platform: platform.into(),
            address: addr1,
            source: WalletSource::GitHubProfileRepo {
                login: login.into(),
                branch: "main".into(),
            },
            recorded_at: Utc.timestamp(1000, 0),
        };
        let e2 = WalletHistoryEntry {
            login: login.into(),
            platform: platform.into(),
            address: addr2,
            source: WalletSource::GitHubProfileRepo {
                login: login.into(),
                branch: "main".into(),
            },
            recorded_at: Utc.timestamp(1001, 0),
        };

        db.append_wallet_history(&e1).unwrap();
        db.append_wallet_history(&e2).unwrap();

        let history = db.get_wallet_history(platform, login).unwrap();
        assert_eq!(history.len(), 2);
        assert!(history[0].recorded_at <= history[1].recorded_at);
    }

    #[test]
    fn wallet_index_multiple_logins() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().to_str().unwrap()).unwrap();
        let platform = "github";
        let address = addr();

        let l1 = WalletLoginLink {
            wallet: address.clone(),
            platform: platform.into(),
            login: "u1".into(),
            linked_at: Utc::now(),
        };
        let l2 = WalletLoginLink {
            wallet: address.clone(),
            platform: platform.into(),
            login: "u2".into(),
            linked_at: Utc::now(),
        };

        db.replace_wallet_link(&l1).unwrap();
        db.replace_wallet_link(&l2).unwrap();

        let links = db.get_logins_for_wallet(&address, platform).unwrap();
        let logins: std::collections::HashSet<_> =
            links.iter().map(|l| l.login.as_str()).collect();
        assert_eq!(logins, ["u1", "u2"].into_iter().collect());
    }

    #[test]
    fn replace_wallet_link_updates_value() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().to_str().unwrap()).unwrap();
        let platform = "github";
        let address = addr();

        let early = Utc.timestamp(2000, 0);
        let late = Utc.timestamp(3000, 0);

        let mut link = WalletLoginLink {
            wallet: address.clone(),
            platform: platform.into(),
            login: "user".into(),
            linked_at: early,
        };
        db.replace_wallet_link(&link).unwrap();
        link.linked_at = late;
        db.replace_wallet_link(&link).unwrap();

        let links = db.get_logins_for_wallet(&address, platform).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].login, "user");
        assert_eq!(links[0].linked_at, late);
    }

    #[test]
    fn batch_writes_are_persisted() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().to_str().unwrap()).unwrap();
        let login = "dora";
        let platform = "github";
        let address = addr();

        let now = Utc::now();
        let uw = UserWallet {
            login: login.into(),
            platform: platform.into(),
            address: address.clone(),
            source: WalletSource::GitHubProfileRepo {
                login: login.into(),
                branch: "main".into(),
            },
            synced_at: now,
        };
        let he = WalletHistoryEntry {
            login: login.into(),
            platform: platform.into(),
            address: address.clone(),
            source: uw.source.clone(),
            recorded_at: now,
        };
        let wl = WalletLoginLink {
            wallet: address.clone(),
            platform: platform.into(),
            login: login.into(),
            linked_at: now,
        };

        let mut batch = db.keyspace.batch();
        db.upsert_user_wallet_batch(&mut batch, &uw).unwrap();
        db.append_wallet_history_batch(&mut batch, &he).unwrap();
        db.replace_wallet_link_batch(&mut batch, &wl).unwrap();
        batch.commit().unwrap();

        assert!(db.get_user_wallet(platform, login).unwrap().is_some());
        assert_eq!(db.get_wallet_history(platform, login).unwrap().len(), 1);
        assert_eq!(
            db.get_logins_for_wallet(&address, platform).unwrap().len(),
            1
        );
    }
}
