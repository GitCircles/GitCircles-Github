This file provides guidance to AI-Agents  when working with code in this repository.

## Common Commands

### Building and Testing
```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run the project
cargo run

# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Check code without building
cargo check
```

### Code Quality
```bash
# Format code
cargo fmt --all

# Run clippy linter
cargo clippy

# Run clippy with all warnings as errors
cargo clippy -- -D warnings
```

## Project Overview

GitCircles-Github is a Rust-based GitHub adapter for the GitCircles ecosystem -
an automated system that tracks Git contributions and distributes cryptocurrency
rewards to contributors.

This component handles GitHub integration: collecting merged PRs, managing API
authentication, tracking base branch changes, and supporting future GitHub App
evolution.

### Current Structure

- `src/main.rs`: Main application entry point and command routing
- `src/lib.rs`: Module definitions and re-exports  
- `src/types.rs`: Core data structures and error handling
- `src/database.rs`: fjall database layer with CRUD operations
- `src/github.rs`: GitHub API client wrapper with pagination
- `src/cli.rs`: Command-line interface and display formatting
- `Cargo.toml`: Project configuration and dependencies

### Dependencies

All required dependencies are implemented:
- ✅ `octocrab`: GitHub API client
- ✅ `fjall`: Embedded database for persistence
- ✅ `clap`: Command-line interface with derive macros
- ✅ `serde`: JSON serialization with datetime support
- ✅ `chrono`: DateTime handling with UTC timezone  
- ✅ `comfy-table`: Formatted table output
- ✅ `indicatif`: Progress bars and spinners

# Current Status

## Recently Completed

### ✅ Issue #7 - Wallet Address Fetching (MERGED)
- All types, validation, and data models
- Database layer with 3 wallet partitions (user_wallets, user_wallet_history, wallet_index)
- GitHub API wallet fetching with branch fallback
- Wallet service with change detection
- 4 wallet commands: sync, show, history, lookup
- Test-token command for debugging authentication

### ✅ PR #6 - Project/ProjectOwner Persistence (MERGED)
- Project management infrastructure
- Database layer with 2 project partitions (projects, project_owners)
- 6 project commands: create, list, show, delete, add-owner, remove-owner
- Project-scoped repository tracking

### Merge Status
- Successfully merged upstream/main (PR #6) into issue-7-clean branch
- All conflicts resolved in: src/cli.rs, src/database.rs, src/main.rs, src/types.rs
- All 16 tests passing (100%)
- Build successful

## Available Commands

### Core Commands
- `init` - Initialize local database
- `collect --repo <owner/repo> [--base-branch main] [--days N] [--project-id ID]` - Collect merged PRs
- `status [--project-id ID]` - Show status of tracked repositories and projects
- `test-token [--token TOKEN]` - Test GitHub token authentication

### Project Management
- `project create <name> [--description TEXT]` - Create a new project
- `project list` - List all projects
- `project show <project-id>` - Show detailed project information
- `project delete <project-id>` - Delete a project
- `project add-owner <project-id> <username> [--role ROLE]` - Add project owner (roles: owner, admin, member)
- `project remove-owner <project-id> <username>` - Remove project owner

### Wallet Management
- `wallet sync <login> [--token TOKEN]` - Fetch and sync wallet address from GitHub
- `wallet show <login>` - Display current wallet info for a user
- `wallet history <login>` - Show wallet change history
- `wallet lookup <address>` - Find all logins associated with a wallet address

### Usage Examples

```bash
# Test authentication
export GITHUB_TOKEN=ghp_your_token_here
cargo run -- test-token

# Create a project and collect PRs
cargo run -- project create "My Project" --description "Description here"
cargo run -- collect --repo owner/repo --project-id my-project_12345

# Sync wallet for a GitHub user
cargo run -- wallet sync <github-login>

# Show project status
cargo run -- status --project-id my-project_12345

# Run all tests
cargo test
```

### Wallet Address Requirements

To use wallet features, users must:
1. Create a public repository named `gitcircles-profile`
2. Add a file named `P2PK.pub` containing their Ergo wallet address
3. The address must be in P2PK format (starting with `9`, 51 characters)

Example: `9hQb8QxZ4gsgAWtGvqh3HPpYCexEQhVsWM4QBQ3AFhSVERPfoM5`

# Specification

## Implementation Components

### 1. GitHub Client Wrapper

**Core responsibilities:**

- Authenticate with GitHub API using PAT
- Fetch closed PRs filtered by base branch
- Handle pagination (GitHub returns max 100 per page)
- Filter results to only merged PRs (`merged_at != null`)
- Rate limit handling with progress feedback

**Key operations:**

- Store/retrieve repository configs
- Upsert merged PRs (handle duplicates)
- Query PRs by repo/time range
- Track base branch changes over time

**API strategy:**
```
GET /repos/{owner}/{repo}/pulls?state=closed&base={branch}&per_page=100&page={n}
```

### 2. Database Layer (fjall DAO) ✅

**Implemented Tables:**
- ✅ `repositories` - Track repo configs, sync status, and PR counts
- ✅ `pull_requests` - Store merged PR data with deduplication  
- ✅ `base_branch_history` - Track base branch changes over time

**Operations Available:**
- ✅ Repository CRUD with `upsert_repository()`, `get_repository()`, `list_repositories()`
- ✅ PR storage with `upsert_pull_request()`, `pull_request_exists()`, `get_pull_requests()`
- ✅ Base branch tracking with `record_base_branch_change()`, `get_base_branch_history()`

**Multi-Project Extensions (Complete):**
- ✅ `projects` - Project configurations and metadata
- ✅ `project_owners` - Many-to-many relationship between projects and owners
- ✅ Updated `repositories` to link to projects
- ✅ Project-scoped PR aggregation with `get_pull_requests_for_project()`

**Wallet Address Tracking (Complete):**
- ✅ `user_wallets` - Current wallet address per platform/login
- ✅ `user_wallet_history` - Complete audit trail of wallet changes
- ✅ `wallet_index` - Reverse lookup from wallet address to logins
- ✅ Atomic batch operations for wallet updates
- ✅ Change detection to avoid unnecessary writes

### 3. CLI Commands Implementation ✅

**`collect` command:** ✅
1. ✅ Validate repo format (`owner/repo`)
2. ✅ Initialize GitHub client with token (supports env var)
3. ✅ Show spinner while fetching with progress messages
4. ✅ Paginate through all closed PRs on base branch
5. ✅ Filter merged PRs, deduplicate, and store in database  
6. ✅ Display results with comfy-table formatting
7. ✅ Handle time range filtering with `--days` parameter
8. ✅ Track base branch changes and repository metadata
9. ✅ Accept `--project-id` parameter for project association

**`status` command:** ✅
- ✅ Query database for all tracked repositories
- ✅ Show last sync time, total PR counts, first sync date
- ✅ Display in formatted table with repository details
- ✅ Accept `--project-id` parameter for project-specific views
- ✅ Multi-project overview with projects and repositories

**`init` command:** ✅
- ✅ Create fjall database structure at standard location
- ✅ Initialize all required partitions/keyspaces

**`project` command:** ✅
- ✅ `create <name> [--description]` - Create new projects
- ✅ `list` - List all projects with metadata
- ✅ `show <project-id>` - Detailed project view with owners and repositories
- ✅ `delete <project-id>` - Delete projects (with safety checks)
- ✅ `add-owner <project-id> <username> [--role]` - Add project owners
- ✅ `remove-owner <project-id> <username>` - Remove project owners

**`wallet` command:** ✅
- ✅ `sync <login> [--token]` - Fetch and sync wallet address from `<login>/gitcircles-profile`
- ✅ `show <login>` - Display current wallet info for a user
- ✅ `history <login>` - Show complete wallet change history
- ✅ `lookup <address>` - Reverse lookup: find all logins using a wallet address

**`test-token` command:** ✅
- ✅ Verify GitHub token authentication by fetching authenticated user info
- ✅ Helpful troubleshooting output with links to create tokens

### 4. Progress & Display ✅

- ✅ Spinner during API calls with progress messages and page indicators
- ✅ Table output showing: PR#, Title (truncated), Author, Merged Date, Base Branch, Commit SHA (short)
- ✅ Repository status table with: Owner/Name, Last Sync, Total PRs, First Sync
- ✅ Error handling with user-friendly messages and proper exit codes
- ✅ Emoji indicators for status updates and branch changes

## Notes About Database Design — 21 Sep 2025

- **High — `src/database.rs:24,46,64,86,109,143`**: Each write forces `PersistMode::SyncAll`, so we hit disk on every insert. This kills throughput (especially during PR collection). Need batching/async persistence or caller-controlled flush.
- **Medium — `src/database.rs:105`**: `get_projects_for_owner` scans the whole `project_owners` partition because keys are `owner:{project}:{username}`; no username prefix. Add an index or reshape keys (`owner_by_user:{username}:{project}`) to make lookups bounded.
- **Medium — `src/database.rs:94`**: `list_repositories_for_project` filters all repos in memory. Key layout (`repo:{project_id}:{owner}/{name}`) or a project→repo index would allow direct prefix scans.
- **Low — `src/database.rs:70`**: `record_base_branch_change` keys include only the seconds timestamp, so two changes inside one second collide. Use higher-resolution timestamps or append a nonce/UUID.

### Current Layout (ASCII)

```
+--------------------+
| fjall keyspace     |
|   gitcircles/db    |
+--------------------+
        |
        +-- Partition: repositories
        |      Key: repo:{owner}/{repo}
        |      Val: { owner, name, current_base_branch,
        |             last_sync, total_prs, first_sync,
        |             project_id? }
        |
        +-- Partition: pull_requests
        |      Key: pr:{owner}/{repo}:{pr_number}
        |      Val: { number, title, author, merged_at,
        |             base_branch, merge_commit_sha, repository }
        |
        +-- Partition: base_branch_history
        |      Key: base:{owner}/{repo}:{timestamp_secs}
        |      Val: { repository, old_branch, new_branch, changed_at }
        |
        +-- Partition: projects
        |      Key: project:{project_id}
        |      Val: { id, name, description?, created_at, updated_at }
        |
        +-- Partition: project_owners
        |      Key: owner:{project_id}:{github_username}
        |      Val: { project_id, github_username, role, added_at }
        |
        +-- Partition: user_wallets
        |      Key: login:{platform}:{login}
        |      Val: { login, platform, address, source, synced_at }
        |
        +-- Partition: user_wallet_history
        |      Key: history:{platform}:{login}:{timestamp_nanos}
        |      Val: { login, platform, address, source, recorded_at }
        |
        +-- Partition: wallet_index
               Key: wallet:{address}:{platform}:{login}
               Val: { wallet, platform, login, linked_at }

Relationships:
- `repositories.project_id` → `projects.id`
- `project_owners.project_id` → `projects.id`
- `pull_requests.repository` references `repositories` via `{owner}/{repo}`
- `pull_requests.author` can be looked up via `user_wallets` using `login:{platform}:{author}`
- `base_branch_history.repository` uses same `{owner}/{repo}` reference
- `user_wallet_history` provides audit trail for `user_wallets`
- `wallet_index` enables reverse lookup from wallet address to all associated logins
```

## Feature Summary

### Issue #7: Wallet Address Tracking ✅ MERGED

Fetches Ergo wallet addresses from user GitHub repositories and maintains bidirectional
mapping between GitHub logins and wallet addresses with full history tracking.

**Implementation:**
- Wallet fetching from `<login>/gitcircles-profile` repository (`src/github.rs`)
- Database partitions: user_wallets, user_wallet_history, wallet_index (`src/database.rs`)
- Wallet service with atomic batch writes and change detection (`src/wallet.rs`)
- CLI commands: sync, show, history, lookup (`src/main.rs`)
- Test-token command for authentication debugging

**Design:**
- Wallet addresses serve as internal user identifiers across platforms
- Address changes indicate potential account ownership transfer
- Multiple logins can share the same wallet address (supported)
- History preserved permanently for audit purposes

### PR #6: Project Management ✅ MERGED

Multi-project support with project ownership and repository grouping.

**Implementation:**
- Database partitions: projects, project_owners (`src/database.rs`)
- CLI commands: create, list, show, delete, add-owner, remove-owner (`src/main.rs`)
- Project-scoped repository tracking and PR aggregation
- Role-based ownership: owner, admin, member

**Design:**
- Projects can have multiple owners with different roles
- Repositories can be associated with projects
- Project deletion requires all repositories to be unlinked first (safety check)
