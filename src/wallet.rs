use chrono::Utc;

use crate::database::Database;
use crate::github::WalletFetcher;
use crate::types::{
    Result, UserWallet, WalletHistoryEntry, WalletLoginLink, WalletSource,
    WalletSyncResult,
};

pub struct WalletService<'a, F: WalletFetcher> {
    db: &'a Database,
    fetcher: &'a F,
}

impl<'a, F: WalletFetcher> WalletService<'a, F> {
    pub fn new(db: &'a Database, fetcher: &'a F) -> Self {
        Self { db, fetcher }
    }

    pub async fn sync_github_login(
        &self,
        login: &str,
    ) -> Result<Option<WalletSyncResult>> {
        // Step 1: Fetch from GitHub
        let outcome = match self.fetcher.fetch_wallet_address(login).await? {
            Some(o) => o,
            None => return Ok(None),
        };

        // Step 2: Get existing wallet
        let previous_wallet = self.db.get_user_wallet("github", login)?;
        let previous_address = previous_wallet.as_ref().map(|w| w.address.clone());

        // Step 3: Detect changes
        let changed = previous_address.as_ref() != Some(&outcome.address);

        // Step 4: Persist only if changed
        if changed {
            let now = Utc::now();

            let user_wallet = UserWallet {
                login: login.to_string(),
                platform: "github".to_string(),
                address: outcome.address.clone(),
                source: WalletSource::GitHubProfileRepo {
                    login: login.to_string(),
                    branch: outcome.branch.clone(),
                },
                synced_at: now,
            };

            let history_entry = WalletHistoryEntry {
                login: login.to_string(),
                platform: "github".to_string(),
                address: outcome.address.clone(),
                source: user_wallet.source.clone(),
                recorded_at: now,
            };

            let wallet_link = WalletLoginLink {
                wallet: outcome.address.clone(),
                platform: "github".to_string(),
                login: login.to_string(),
                linked_at: now,
            };

            // Atomic batch write
            let mut batch = self.db.keyspace.batch();
            self.db.upsert_user_wallet_batch(&mut batch, &user_wallet)?;
            self.db
                .append_wallet_history_batch(&mut batch, &history_entry)?;
            self.db
                .replace_wallet_link_batch(&mut batch, &wallet_link)?;
            batch.commit()?;
        }

        // Step 5: Return result
        Ok(Some(WalletSyncResult {
            current: outcome.address,
            previous: previous_address,
            changed,
            source: WalletSource::GitHubProfileRepo {
                login: login.to_string(),
                branch: outcome.branch,
            },
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::WalletFetcher;
    use crate::types::{WalletAddress, WalletFetchOutcome};
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use tempfile::tempdir;

    struct MockFetcher {
        outcomes: Mutex<VecDeque<Option<WalletFetchOutcome>>>,
    }

    impl WalletFetcher for MockFetcher {
        async fn fetch_wallet_address(
            &self,
            _login: &str,
        ) -> Result<Option<WalletFetchOutcome>> {
            let mut lock = self.outcomes.lock().unwrap();
            Ok(lock.pop_front().unwrap_or(None))
        }
    }

    fn addr(suffix: &str) -> WalletAddress {
        // Construct a valid P2PK-like address: '9' + 50 allowed chars
        let base = format!("9{}{}", "A".repeat(50 - suffix.len()), suffix);
        WalletAddress::try_from(base.as_str()).unwrap()
    }

    #[tokio::test]
    async fn first_sync_changes_and_persists() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().to_str().unwrap()).unwrap();

        let outcome = WalletFetchOutcome {
            address: addr("X"),
            branch: "main".to_string(),
        };
        let fetcher = MockFetcher {
            outcomes: Mutex::new(VecDeque::from([Some(outcome)])),
        };

        let service = WalletService::new(&db, &fetcher);
        let res = service.sync_github_login("alice").await.unwrap().unwrap();

        assert!(res.changed);
        let stored = db.get_user_wallet("github", "alice").unwrap().unwrap();
        assert_eq!(stored.address, res.current);

        let history = db.get_wallet_history("github", "alice").unwrap();
        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn resync_same_address_no_change_no_write() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().to_str().unwrap()).unwrap();

        let a1 = WalletFetchOutcome {
            address: addr("X"),
            branch: "main".to_string(),
        };
        let a2 = WalletFetchOutcome {
            address: addr("X"),
            branch: "main".to_string(),
        };
        let fetcher = MockFetcher {
            outcomes: Mutex::new(VecDeque::from([Some(a1), Some(a2)])),
        };

        let service = WalletService::new(&db, &fetcher);
        let first = service.sync_github_login("bob").await.unwrap().unwrap();
        assert!(first.changed);

        let before_history = db.get_wallet_history("github", "bob").unwrap().len();
        let second = service.sync_github_login("bob").await.unwrap().unwrap();
        assert!(!second.changed);
        let after_history = db.get_wallet_history("github", "bob").unwrap().len();
        assert_eq!(before_history, after_history);
    }

    #[tokio::test]
    async fn address_change_appends_history_and_updates_index() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().to_str().unwrap()).unwrap();

        let a1 = WalletFetchOutcome {
            address: addr("X"),
            branch: "main".to_string(),
        };
        let a2 = WalletFetchOutcome {
            address: addr("Y"),
            branch: "main".to_string(),
        };
        let fetcher = MockFetcher {
            outcomes: Mutex::new(VecDeque::from([
                Some(a1.clone()),
                Some(a2.clone()),
            ])),
        };

        let service = WalletService::new(&db, &fetcher);
        let _ = service.sync_github_login("carol").await.unwrap();

        // Sleep for 1 second to ensure timestamps differ
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let res2 = service.sync_github_login("carol").await.unwrap().unwrap();

        assert!(res2.changed);

        let history = db.get_wallet_history("github", "carol").unwrap();
        assert_eq!(history.len(), 2);

        // Index should resolve login for both old and new addresses
        let old_links = db.get_logins_for_wallet(&a1.address, "github").unwrap();
        assert!(old_links.iter().any(|l| l.login == "carol"));

        let new_links = db.get_logins_for_wallet(&a2.address, "github").unwrap();
        assert!(new_links.iter().any(|l| l.login == "carol"));
    }
}
