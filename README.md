# GitCircles-Github

A Rust-based GitHub adapter for the GitCircles ecosystem - an automated system that tracks Git contributions and distributes cryptocurrency rewards to contributors.

This component handles GitHub integration: collecting merged pull requests, managing project structures, tracking wallet addresses, and providing the foundation for automated contributor rewards.

## Features

- **Pull Request Tracking**: Collect and store merged PRs from GitHub repositories
- **Project Management**: Organize repositories into projects with multi-owner support
- **Wallet Integration**: Link GitHub accounts to Ergo cryptocurrency wallet addresses
- **Base Branch Tracking**: Monitor and record changes to repository base branches
- **Local Database**: Embedded fjall database for fast, reliable data persistence

## Installation

### Prerequisites

- Rust 1.70 or later
- GitHub Personal Access Token (PAT) with `repo` or `public_repo` scope

### Build from Source

```bash
git clone https://github.com/yourusername/GitCircles-Github.git
cd GitCircles-Github
cargo build --release
```

The binary will be available at `target/release/gitcircles-github`.

## Quick Start

### 1. Initialize the Database

```bash
export GITHUB_TOKEN=ghp_your_token_here
./gitcircles-github init
```

### 2. Verify Your Token

```bash
./gitcircles-github test-token
```

### 3. Collect Pull Requests

```bash
# Collect PRs from a repository
./gitcircles-github collect --repo owner/repo

# Collect PRs from the last 30 days
./gitcircles-github collect --repo owner/repo --days 30

# Collect PRs from a specific base branch
./gitcircles-github collect --repo owner/repo --base-branch develop
```

### 4. Check Status

```bash
./gitcircles-github status
```

## Commands

### Core Commands

#### `init`
Initialize the local database.

```bash
gitcircles-github init
```

#### `collect`
Collect merged pull requests from a GitHub repository.

```bash
gitcircles-github collect --repo <owner/repo> [OPTIONS]

Options:
  -r, --repo <REPO>              Repository in format "owner/repo"
  -t, --token <TOKEN>            GitHub personal access token (or use GITHUB_TOKEN env var)
  -b, --base-branch <BRANCH>     Target base branch [default: main]
  -d, --days <DAYS>              Number of days to look back
  -p, --project-id <PROJECT_ID>  Associate repository with a project
```

**Examples:**
```bash
# Collect all merged PRs
gitcircles-github collect --repo rust-lang/rust

# Collect recent PRs
gitcircles-github collect --repo owner/repo --days 30

# Associate with a project
gitcircles-github collect --repo owner/repo --project-id my-project_12345
```

#### `status`
Show status of tracked repositories and projects.

```bash
gitcircles-github status [OPTIONS]

Options:
  -p, --project-id <PROJECT_ID>  Show status for specific project only
```

**Examples:**
```bash
# Show all repositories and projects
gitcircles-github status

# Show specific project
gitcircles-github status --project-id my-project_12345
```

#### `test-token`
Test GitHub token authentication.

```bash
gitcircles-github test-token [OPTIONS]

Options:
  -t, --token <TOKEN>  GitHub personal access token (or use GITHUB_TOKEN env var)
```

### Project Management

#### `project create`
Create a new project.

```bash
gitcircles-github project create <NAME> [OPTIONS]

Arguments:
  <NAME>  Project name

Options:
  -d, --description <TEXT>  Project description
```

**Example:**
```bash
gitcircles-github project create "My Open Source Project" \
  --description "A collection of related repositories"
```

#### `project list`
List all projects.

```bash
gitcircles-github project list
```

#### `project show`
Show detailed information about a project.

```bash
gitcircles-github project show <PROJECT_ID>

Arguments:
  <PROJECT_ID>  Project ID
```

#### `project delete`
Delete a project (requires all repositories to be unlinked first).

```bash
gitcircles-github project delete <PROJECT_ID>

Arguments:
  <PROJECT_ID>  Project ID
```

#### `project add-owner`
Add an owner to a project.

```bash
gitcircles-github project add-owner <PROJECT_ID> <USERNAME> [OPTIONS]

Arguments:
  <PROJECT_ID>  Project ID
  <USERNAME>    GitHub username

Options:
  -r, --role <ROLE>  Role: owner, admin, or member [default: member]
```

**Example:**
```bash
gitcircles-github project add-owner my-project_12345 alice --role owner
```

#### `project remove-owner`
Remove an owner from a project.

```bash
gitcircles-github project remove-owner <PROJECT_ID> <USERNAME>

Arguments:
  <PROJECT_ID>  Project ID
  <USERNAME>    GitHub username
```

### Wallet Management

#### `wallet sync`
Fetch and sync wallet address for a GitHub user.

Reads `P2PK.pub` from the user's `gitcircles-profile` repository.

```bash
gitcircles-github wallet sync <LOGIN> [OPTIONS]

Arguments:
  <LOGIN>  GitHub username

Options:
  -t, --token <TOKEN>  GitHub personal access token (or use GITHUB_TOKEN env var)
```

**Example:**
```bash
gitcircles-github wallet sync alice
```

#### `wallet show`
Show current wallet address for a GitHub user.

```bash
gitcircles-github wallet show <LOGIN>

Arguments:
  <LOGIN>  GitHub username
```

#### `wallet history`
Show wallet address history for a GitHub user.

```bash
gitcircles-github wallet history <LOGIN>

Arguments:
  <LOGIN>  GitHub username
```

#### `wallet lookup`
Find all GitHub logins associated with a wallet address.

```bash
gitcircles-github wallet lookup <WALLET>

Arguments:
  <WALLET>  Wallet address
```

**Example:**
```bash
gitcircles-github wallet lookup 9hQb8QxZ4gsgAWtGvqh3HPpYCexEQhVsWM4QBQ3AFhSVERPfoM5
```

## Wallet Address Setup

To enable wallet tracking, users must create a public GitHub repository with their wallet address:

1. Create a public repository named `gitcircles-profile`
2. Add a file named `P2PK.pub` at the repository root
3. Put your Ergo P2PK wallet address in the file (single line, no extra whitespace)

**Format Requirements:**
- Must start with `9`
- Exactly 51 characters
- Base58 encoding (no ambiguous characters like 0, O, I, l)

**Example:**
```
9hQb8QxZ4gsgAWtGvqh3HPpYCexEQhVsWM4QBQ3AFhSVERPfoM5
```

The system will try branches in this order: `main`, `master`, then the repository's default branch.

## Environment Variables

- `GITHUB_TOKEN`: GitHub Personal Access Token for API authentication
- `HOME` or `USERPROFILE`: Used to determine database location (`~/.gitcircles/db`)

## Database Schema

The application uses an embedded fjall key-value database with the following partitions:

### Core Data
- **repositories**: Repository tracking and sync status
- **pull_requests**: Merged pull request data
- **base_branch_history**: Base branch change tracking

### Projects
- **projects**: Project metadata and configuration
- **project_owners**: Project ownership with role-based access

### Wallets
- **user_wallets**: Current wallet address per user
- **user_wallet_history**: Complete audit trail of wallet changes
- **wallet_index**: Reverse lookup from wallet to logins

Database location: `~/.gitcircles/db`

## Development

### Running Tests

```bash
cargo test
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Run linter
cargo clippy

# Run linter with warnings as errors
cargo clippy -- -D warnings
```

### Building for Release

```bash
cargo build --release
```

## Architecture

GitCircles-Github is designed as a modular CLI tool with clear separation of concerns:

- **src/main.rs**: Command routing and application entry point
- **src/cli.rs**: Command-line interface definitions and display formatting
- **src/github.rs**: GitHub API client wrapper with pagination
- **src/database.rs**: fjall database layer with CRUD operations
- **src/wallet.rs**: Wallet sync service with change detection
- **src/types.rs**: Core data structures and error handling

## Contributing

Contributions are welcome! Please ensure:

1. All tests pass: `cargo test`
2. Code is formatted: `cargo fmt --all`
3. Clippy checks pass: `cargo clippy -- -D warnings`
4. New features include tests

## License

[Add your license here]

## Roadmap

- [ ] GitHub App integration for automatic webhook-based PR collection
- [ ] Multi-platform support (GitLab, Bitbucket)
- [ ] Advanced contribution metrics and analytics
- [ ] Automated reward distribution integration
- [ ] Web dashboard for visualization

## Support

For issues and feature requests, please use the [GitHub Issues](https://github.com/yourusername/GitCircles-Github/issues) page.
