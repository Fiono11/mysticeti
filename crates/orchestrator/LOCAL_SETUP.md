# Running Benchmarks Locally

This guide explains how to run the orchestrator benchmarks on your local machine without using cloud instances.

## Overview

The orchestrator supports local execution through a `local` cloud provider. This creates virtual instances that all point to `localhost` (127.0.0.1), allowing you to run benchmarks on your local machine. **No SSH setup is required** - commands are executed directly on your local machine.

## Prerequisites

1. **Rust and dependencies**: Make sure you have Rust installed and can build the project
2. **Shell access**: A Unix-like shell (sh) is required for command execution

## Quick Start: Using the Script

The easiest way to run local benchmarks is using the provided script:

```bash
# Quick local test (default: 4 nodes, 200 tx/s)
./scripts/local-benchmark.sh

# Run with custom parameters
./scripts/local-benchmark.sh --committee 2 --loads 100

# Run with multiple loads
./scripts/local-benchmark.sh --committee 4 --loads 100 --loads 200 --loads 500

# Use local project code instead of cloning from git (recommended for development)
./scripts/local-benchmark.sh --committee 4 --loads 200 --use-local-code

# Skip update and config for faster iteration
./scripts/local-benchmark.sh --committee 4 --loads 200 --skip-update --skip-config
```

The script automatically:
- Copies `settings-local.yml` to `settings.yml`
- Verifies the configuration
- Optionally sets up a symlink to use local code (with `--use-local-code`)
- Optionally deploys instances
- Runs the benchmark with your specified parameters

**Tip:** Use `--use-local-code` to test your local changes without cloning from git. This creates a symlink so the orchestrator uses your current project directory.

For more details, run:
```bash
./scripts/local-benchmark.sh --help
```

## Manual Setup: Step-by-Step Guide

If you prefer to run commands manually or need more control, follow these steps:

## Step 1: Configure Local Settings

1. **Copy the local settings template**:
   ```bash
   cp crates/orchestrator/assets/settings-local.yml crates/orchestrator/assets/settings.yml
   ```

2. **Edit the settings file** to match your environment:
   - Update `working_dir` to where you want the code to be cloned (default: `~/mysticeti-working`)
   - Adjust `benchmark_duration`, `monitoring`, etc. as needed
   - The `ssh_private_key_file` field is not used for local execution but must be present

3. **Important**: The `cloud_provider` must be set to `local` in your settings file.

## Step 2: Run Benchmarks

**Note:** For local execution, you don't need to deploy instances manually. The benchmark command will automatically create the needed instances if none exist.

If you want to deploy instances manually first (optional):

```bash
cargo run --bin orchestrator -- testbed deploy --instances 4
```

This creates 4 virtual instances, all pointing to localhost. Since they're all the same machine, you're limited by your local machine's resources.

Run a benchmark with a committee of 4 nodes and a load of 200 tx/s:

```bash
cargo run --bin orchestrator -- benchmark --committee 4 --loads 200
```

### Options for Local Development

When developing locally, you may want to skip some steps:

- **Skip testbed update** (if code is already built):
  ```bash
  cargo run --bin orchestrator -- benchmark --committee 4 --loads 200 --skip-testbed-update
  ```

- **Skip testbed configuration** (if config is already set up):
  ```bash
  cargo run --bin orchestrator -- benchmark --committee 4 --loads 200 --skip-testbed-configuration
  ```

## Limitations

1. **Single Machine**: All instances run on the same machine, so you're limited by your local resources (CPU, memory, network)

2. **Port Conflicts**: Multiple nodes and clients on the same machine may conflict on ports. The protocol should handle this, but be aware.

3. **Performance**: Local benchmarks won't reflect real network conditions (latency, bandwidth) that you'd see in a distributed setup.

## Troubleshooting

### Command Execution Fails

- Make sure you have a Unix-like shell (sh) available
- Check that the `working_dir` in settings.yml exists and is writable
- Verify you have permissions to execute commands in the working directory

### Permission Denied

- Ensure the working directory has correct permissions: `chmod 755 ~/mysticeti-working`
- Make sure you have write access to the working directory

### Port Already in Use

If you see port conflicts, you may need to:
- Kill existing processes using those ports
- Adjust the protocol configuration to use different ports
- Reduce the number of instances

## Example: Quick Local Test

### Using the Script (Recommended)

```bash
# Quick test with 2 nodes and 100 tx/s (uses local code)
./scripts/local-benchmark.sh --committee 2 --loads 100 --use-local-code

# Or without local code (clones from git)
./scripts/local-benchmark.sh --committee 2 --loads 100 --skip-update
```

### Manual Method

```bash
# 1. Use local settings (no SSH setup needed)
cp crates/orchestrator/assets/settings-local.yml crates/orchestrator/assets/settings.yml

# 2. Run a quick benchmark (instances will be created automatically)
cargo run --bin orchestrator -- benchmark --committee 2 --loads 100 --skip-testbed-update
```

That's it! The benchmark command will automatically create the needed local instances.

## Next Steps

Once you have local execution working, you can:
- Test protocol changes quickly
- Debug configuration issues
- Develop and test new features
- Run smaller-scale benchmarks

For production-scale benchmarks with realistic network conditions, use AWS or Vultr cloud providers as described in the main README.

