use chrono::Utc;

use crate::types::{
    BaseBranchChange, MergedPullRequest, Repository, Result, UserWallet,
    WalletAddress, WalletHistoryEntry, WalletLoginLink,
};

pub struct Database {
    pub keyspace: fjall::Keyspace,
    repositories: fjall::PartitionHandle,
    pull_requests: fjall::PartitionHandle,
    base_branch_history: fjall::PartitionHandle,
    user_wallets: fjall::PartitionHandle,
    user_wallet_history: fjall::PartitionHandle,
    wallet_index: fjall::PartitionHandle,
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

        Ok(Self {
            keyspace,
            repositories,
            pull_requests,
            base_branch_history,
            user_wallets,
            user_wallet_history,
            wallet_index,
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
        if let Some(value) = self.user_wallets.get(&key)? {
            let wallet: UserWallet = serde_json::from_slice(&value)?;
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
        let mut entries = Vec::new();
        let prefix = format!("history:{}:{}:", platform, login);
        for item in self.user_wallet_history.prefix(prefix.as_bytes()) {
            let (_, value) = item?;
            let entry: WalletHistoryEntry = serde_json::from_slice(&value)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    pub fn get_logins_for_wallet(
        &self,
        address: &WalletAddress,
        platform: &str,
    ) -> Result<Vec<WalletLoginLink>> {
        let mut links = Vec::new();
        let prefix = format!("wallet:{}:{}:", address, platform);
        for item in self.wallet_index.prefix(prefix.as_bytes()) {
            let (_, value) = item?;
            let link: WalletLoginLink = serde_json::from_slice(&value)?;
            links.push(link);
        }
        Ok(links)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WalletSource;
    use chrono::TimeZone;
    use tempfile::tempdir;

    fn addr(suffix: &str) -> WalletAddress {
        let base = format!("9{}{}", "A".repeat(50 - suffix.len()), suffix);
        WalletAddress::try_from(base.as_str()).unwrap()
    }

    #[test]
    fn user_wallet_roundtrip() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().to_str().unwrap()).unwrap();

        let uw = UserWallet {
            login: "alice".into(),
            platform: "github".into(),
            address: addr("X"),
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
        let addr1 = addr("1");
        let addr2 = addr("2");

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
        let address = addr("XYZ");

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
        let address = addr("ZZ");

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
        let address = addr("B");

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
