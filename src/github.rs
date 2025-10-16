use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use octocrab::{Octocrab, Page};
use std::collections::HashSet;
use std::time::Duration;

use crate::types::{
    GitCirclesError, MergedPullRequest, Result, WalletAddress, WalletFetchOutcome,
};

// Minimal, local constants for wallet fetch path
const PROFILE_REPO_NAME: &str = "gitcircles-profile";
const WALLET_FILE_PATH: &str = "P2PK.pub";

pub struct GitHubClient {
    octocrab: Octocrab,
}

// Trait to allow testing Wallet fetch logic without real network
// Implemented by GitHubClient; tests can provide a mock implementation.
#[async_trait::async_trait]
pub trait WalletFetcher: Send + Sync {
    async fn fetch_wallet_address(
        &self,
        login: &str,
    ) -> Result<Option<WalletFetchOutcome>>;
}

impl GitHubClient {
    pub fn new(token: &str) -> Result<Self> {
        let octocrab = Octocrab::builder()
            .personal_token(token.to_string())
            .build()?;

        Ok(Self { octocrab })
    }

    /// Test if the GitHub token is valid by fetching the authenticated user
    pub async fn test_token(&self) -> Result<String> {
        let user = self.octocrab.current().user().await?;
        Ok(user.login)
    }

    pub async fn fetch_merged_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        base_branch: &str,
        days_back: Option<u64>,
    ) -> Result<Vec<MergedPullRequest>> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.green} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.set_message("Fetching pull requests from GitHub...");
        pb.enable_steady_tick(Duration::from_millis(100));

        let mut merged_prs = Vec::new();
        let mut page = 1u32;
        let per_page = 100u8;

        // Calculate cutoff date if days_back is specified
        let cutoff_date =
            days_back.map(|days| Utc::now() - chrono::Duration::days(days as i64));

        loop {
            pb.set_message(format!("Fetching page {} from GitHub API...", page));

            let pulls_page: Page<octocrab::models::pulls::PullRequest> = self
                .octocrab
                .pulls(owner, repo)
                .list()
                .state(octocrab::params::State::Closed)
                .base(base_branch)
                .per_page(per_page)
                .page(page)
                .send()
                .await?;

            let pulls = pulls_page.items;
            if pulls.is_empty() {
                break;
            }

            pb.set_message(format!(
                "Processing {} PRs from page {}...",
                pulls.len(),
                page
            ));
            let pulls_len = pulls.len();

            for pr in pulls {
                // Only include merged PRs
                if let Some(merged_at) = pr.merged_at {
                    // Check if within date range if specified
                    if let Some(cutoff) = cutoff_date
                        && merged_at < cutoff
                    {
                        continue;
                    }

                    let merged_pr = MergedPullRequest {
                        number: pr.number,
                        title: pr.title.unwrap_or_else(|| "No title".to_string()),
                        author: pr
                            .user
                            .map(|u| u.login)
                            .unwrap_or_else(|| "unknown".to_string()),
                        merged_at,
                        base_branch: pr.base.ref_field,
                        merge_commit_sha: pr
                            .merge_commit_sha
                            .unwrap_or_else(|| "unknown".to_string()),
                        repository: format!("{}/{}", owner, repo),
                    };

                    merged_prs.push(merged_pr);
                }
            }

            // If this page wasn't full, we've reached the end
            if pulls_len < per_page as usize {
                break;
            }

            page += 1;
        }

        pb.finish_with_message(format!("✓ Found {} merged PRs", merged_prs.len()));
        Ok(merged_prs)
    }

    pub async fn fetch_wallet_address(
        &self,
        login: &str,
    ) -> Result<Option<WalletFetchOutcome>> {
        let repo_full = format!("{}/{}", login, PROFILE_REPO_NAME);

        // Step 1: Get repository metadata to find default branch
        let repo_result = self.octocrab.repos(login, PROFILE_REPO_NAME).get().await;

        let default_branch = match repo_result {
            Ok(repo) => repo.default_branch.unwrap_or_else(|| "main".to_string()),
            Err(octocrab::Error::GitHub { source, .. })
                if source.message.contains("Not Found") =>
            {
                // Repository doesn't exist - not an error, just means no wallet configured
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };

        // Step 2: Build branch list with deduplication
        let mut branches =
            vec!["main".to_string(), "master".to_string(), default_branch];
        let mut seen = HashSet::new();
        branches.retain(|b| seen.insert(b.clone()));

        // Step 3: Try fetching raw file from each branch
        let client = reqwest::Client::new();

        for branch in &branches {
            let url = format!(
                "https://raw.githubusercontent.com/{}/{}/{}/{}",
                login, PROFILE_REPO_NAME, branch, WALLET_FILE_PATH
            );

            match client.get(&url).send().await {
                Ok(response) => {
                    match response.status().as_u16() {
                        200 => {
                            // Step 4: Validate wallet address
                            let content = response.text().await.map_err(|e| {
                                GitCirclesError::WalletInvalidFormat(
                                    repo_full.clone(),
                                    format!("Failed to read response: {}", e),
                                )
                            })?;

                            let trimmed = content.trim();
                            let address = WalletAddress::try_from(trimmed)?;

                            // Step 5: Return outcome
                            return Ok(Some(WalletFetchOutcome {
                                address,
                                branch: branch.clone(),
                            }));
                        }
                        404 => {
                            // File not found on this branch, try next
                            continue;
                        }
                        401 | 403 => {
                            // Authentication/permission issue - repo must be public
                            return Err(GitCirclesError::RepoNotAccessible(
                                repo_full,
                            ));
                        }
                        status => {
                            // Other unexpected errors
                            return Err(GitCirclesError::WalletInvalidFormat(
                                repo_full,
                                format!("Unexpected HTTP status: {}", status),
                            ));
                        }
                    }
                }
                Err(e) => {
                    // Network or other reqwest errors
                    return Err(GitCirclesError::WalletInvalidFormat(
                        repo_full,
                        format!("Request failed: {}", e),
                    ));
                }
            }
        }

        // All branches returned 404 - file doesn't exist
        Ok(None)
    }
}

#[async_trait::async_trait]
impl WalletFetcher for GitHubClient {
    async fn fetch_wallet_address(
        &self,
        login: &str,
    ) -> Result<Option<WalletFetchOutcome>> {
        Self::fetch_wallet_address(self, login).await
    }
}

// Small helper for testing branch priority logic deterministically without network
#[allow(dead_code)]
pub(crate) fn compute_branch_priority(default_branch: String) -> Vec<String> {
    let mut branches =
        vec!["main".to_string(), "master".to_string(), default_branch];
    let mut seen = std::collections::HashSet::new();
    branches.retain(|b| seen.insert(b.clone()));
    branches
}

#[cfg(test)]
mod tests {
    use super::compute_branch_priority;

    #[test]
    fn branch_priority_dedups_default_main() {
        let branches = compute_branch_priority("main".to_string());
        assert_eq!(branches, vec!["main", "master"]);
    }

    #[test]
    fn branch_priority_includes_custom_default() {
        let branches = compute_branch_priority("develop".to_string());
        assert_eq!(branches, vec!["main", "master", "develop"]);
    }
}
