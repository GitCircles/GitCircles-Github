use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use octocrab::{Octocrab, Page};
use std::time::Duration;

use crate::types::{MergedPullRequest, Result};

pub struct GitHubClient {
    octocrab: Octocrab,
}

impl GitHubClient {
    pub fn new(token: &str) -> Result<Self> {
        let octocrab = Octocrab::builder()
            .personal_token(token.to_string())
            .build()?;

        Ok(Self { octocrab })
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
}
