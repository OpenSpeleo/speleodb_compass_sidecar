# Testing Guide

## Environment Variables for Testing

This project uses environment variables for test credentials, which allows:
- Local testing with a `.env` file
- CI/CD testing with secrets
- No modification of user preferences during tests

## Local Testing Setup

1. **Copy the distribution environment file:**
   ```bash
   cp .env.dist .env
   ```

2. **Edit `.env` with your test credentials:**
   ```bash
   # SpeleoDB instance URL (without trailing slash)
   TEST_SPELEODB_INSTANCE=https://www.speleoDB.org

   # OAuth token (40 character hexadecimal)
   TEST_SPELEODB_OAUTH=your_actual_token_here
   ```

3. **Run the tests:**
   ```bash
   # Test EVERYTHING (Rust tests + WASM UI)
   # All tests use real HTTP requests to your server
   make test
   
   # Or just Rust tests
   make test-rust
   
   # Or directly with cargo
   cargo test --workspace --exclude speleodb-compass-sidecar-ui
   ```

   Or for specific test targets:
   ```bash
   # Test only the Tauri backend
   make test-tauri
   
   # Test only the common crate
   make test-common
   
   # Test only WASM UI
   make test-ui
   
   # Test with output visible
   make test-rust-verbose
   ```

   Tests will:
   - Authenticate with your real API
   - Fetch real project data
   - Test error handling with invalid credentials
   - All using **real HTTP requests** - no mocking

## CI/CD Setup

### GitHub Actions

The project includes two GitHub Actions workflows:

1. **`.github/workflows/ci.yml`** - Runs tests on every push and pull request
   - Executes `make test` (all Rust + WASM UI tests)
   - Runs on Windows (matching the publish environment)
   - Uses caching for faster builds (Rust, cargo bins, trunk artifacts)

2. **`.github/workflows/publish.yml`** - Publishes the app
   - Automatically runs CI tests first (via `needs: test`)
   - Only publishes if all tests pass
   - Creates GitHub releases with built artifacts

### Setting Up Secrets

Set these environment variables as repository secrets in GitHub:

- `TEST_SPELEODB_INSTANCE` - Your SpeleoDB instance URL
- `TEST_SPELEODB_OAUTH` - Your OAuth token for testing

**To add secrets:**
1. Go to your repository → Settings → Secrets and variables → Actions
2. Click "New repository secret"
3. Add `TEST_SPELEODB_INSTANCE` and `TEST_SPELEODB_OAUTH`

If secrets are not set, the workflow will use default placeholder values (tests may fail if they require real API access).

### Workflow Behavior

```
Push to main/master or PR:
  ├─ CI Tests workflow runs automatically
  │  ├─ Setup environment (Node, Rust, wasm-pack, trunk)
  │  ├─ Run `make test`
  │  └─ Report results
  
Push to main/master with tags:
  ├─ CI Tests workflow runs first
  ├─ If tests pass ✓
  │  └─ Publish workflow runs
  │     └─ Build and release artifacts
  └─ If tests fail ✗
     └─ Publish workflow is skipped
```

### Manual Workflow Dispatch

You can also manually trigger the CI workflow from the Actions tab in GitHub.

## How It Works

1. **Tests load `.env` automatically**: The test suite uses the `dotenvy` crate to load environment variables from `.env` before running tests.

2. **Real HTTP requests only**: All tests make actual HTTP requests to your SpeleoDB server. There are no mocks or fake servers.

3. **Fallback to user preferences**: The `fetch_projects` command checks for environment variables first. If not found, it falls back to user preferences (for production use).

4. **No test pollution**: Tests use environment variables instead of saving to user preferences, preventing test data from affecting your local development environment.

## Test Categories

All tests make **real HTTP requests** to your SpeleoDB instance. There are no mocks.

**Requirements:**
- A running SpeleoDB server (set in `TEST_SPELEODB_INSTANCE`)
- Valid OAuth token (set in `TEST_SPELEODB_OAUTH`)

### Test Details

#### Common Crate (8 tests)
- Application directory management
- File logger initialization
- Path utilities

#### Tauri Backend (25 tests)
- Basic commands (greet)
- Token parsing (8 tests)
- User preferences (5 tests) - uses `#[serial]`
- Authentication (6 tests)
- Projects API (3 tests) - uses `#[serial]`

### Running Specific Tests

```bash
# Run only authentication tests
cargo test native_auth_request

# Run only project fetching tests
cargo test fetch_projects

# Run with output
cargo test -- --nocapture

# Run tests serially (slower but prevents race conditions)
cargo test -- --test-threads=1
```

## Makefile Targets

The project includes helpful Makefile targets for common tasks:

### Testing
- `make test` - **Test EVERYTHING** (Rust + WASM UI) - **default** - uses real API
- `make test-rust` - Run only Rust tests (backend + common crate) - uses real API
- `make test-rust-verbose` - Run Rust tests with output visible (`--nocapture`)
- `make test-tauri` - Run only Tauri backend tests - uses real API
- `make test-common` - Run only common crate tests
- `make test-ui` - Run only WASM UI tests (requires wasm-pack)
- `make check-env` - Verify `.env` file exists

### Building
- `make build` - Build the project (legacy flit)
- `make build-tauri` - Build the Tauri application
- `make build-ui` - Build UI for distribution with trunk

### Development
- `make dev` - Run development server with hot-reload
- `make check-env` - Verify `.env` file exists
- `make clean` - Remove build artifacts

## Notes

- `.env` is in `.gitignore` and will never be committed
- `.env.dist` is the distribution template (committed to git)
- Tests that modify shared state use the `#[serial]` attribute to prevent race conditions
- **All tests use real HTTP requests** - no mocks, no fake servers
- Tests require a running SpeleoDB instance at `TEST_SPELEODB_INSTANCE`
