#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# SPDX-License-Identifier: Apache-2.0

# Script to run orchestrator benchmarks locally
# This script automates the local setup process described in LOCAL_SETUP.md

set -e

# Colors for output
RED='\033[1;31m'
GREEN='\033[1;32m'
BLUE='\033[1;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
COMMITTEE=4
LOADS=200
SKIP_UPDATE=false
SKIP_CONFIG=false
SETUP_ONLY=false
DEPLOY_INSTANCES=false
INSTANCES=0
USE_LOCAL_CODE=false

# Script directory (where this script is located)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ORCHESTRATOR_DIR="$PROJECT_ROOT/crates/orchestrator"
SETTINGS_LOCAL="$ORCHESTRATOR_DIR/assets/settings-local.yml"
SETTINGS_FILE="$ORCHESTRATOR_DIR/assets/settings.yml"

# Print usage information
usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Run orchestrator benchmarks locally without cloud instances.

OPTIONS:
    -c, --committee NUM         Committee size (default: 4)
    -l, --loads NUM             Load in tx/s (default: 200)
                                 Can be specified multiple times for multiple loads
    -s, --skip-update           Skip testbed update
    -k, --skip-config           Skip testbed configuration
    -d, --deploy NUM            Deploy NUM instances before running benchmark
    --use-local-code            Use local project code instead of cloning from git
    --setup-only                Only setup settings file, don't run benchmark
    -h, --help                  Show this help message

EXAMPLES:
    # Quick local test (default: 4 nodes, 200 tx/s)
    $0

    # Run with 2 nodes and 100 tx/s
    $0 --committee 2 --loads 100

    # Run with multiple loads
    $0 --committee 4 --loads 100 --loads 200 --loads 500

    # Skip update and config for faster iteration
    $0 --committee 4 --loads 200 --skip-update --skip-config

    # Use local code instead of cloning from git
    $0 --committee 4 --loads 200 --use-local-code

    # Deploy instances first, then run benchmark
    $0 --deploy 4 --committee 4 --loads 200

    # Only setup settings file
    $0 --setup-only

EOF
}

# Parse command line arguments
LOADS_ARGS=()
while [[ $# -gt 0 ]]; do
    case $1 in
        -c|--committee)
            COMMITTEE="$2"
            shift 2
            ;;
        -l|--loads)
            LOADS_ARGS+=("$2")
            shift 2
            ;;
        -s|--skip-update)
            SKIP_UPDATE=true
            shift
            ;;
        -k|--skip-config)
            SKIP_CONFIG=true
            shift
            ;;
        -d|--deploy)
            DEPLOY_INSTANCES=true
            INSTANCES="$2"
            shift 2
            ;;
        --use-local-code)
            USE_LOCAL_CODE=true
            shift
            ;;
        --setup-only)
            SETUP_ONLY=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo -e "${RED}Error: Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

# If loads were specified via --loads, use them; otherwise use default
if [ ${#LOADS_ARGS[@]} -gt 0 ]; then
    LOADS="${LOADS_ARGS[*]}"
fi

# Function to print colored messages
info() {
    echo -e "${BLUE}$1${NC}"
}

success() {
    echo -e "${GREEN}$1${NC}"
}

warning() {
    echo -e "${YELLOW}$1${NC}"
}

error() {
    echo -e "${RED}$1${NC}"
}

# Step 1: Setup settings file
setup_settings() {
    info "Setting up local settings..."
    
    if [ ! -f "$SETTINGS_LOCAL" ]; then
        error "Error: settings-local.yml not found at $SETTINGS_LOCAL"
        exit 1
    fi
    
    if [ -f "$SETTINGS_FILE" ]; then
        warning "settings.yml already exists. Backing up to settings.yml.backup"
        cp "$SETTINGS_FILE" "$SETTINGS_FILE.backup"
    fi
    
    cp "$SETTINGS_LOCAL" "$SETTINGS_FILE"
    success "Settings file configured: $SETTINGS_FILE"
    
    # Verify cloud_provider is set to local
    if ! grep -q "cloud_provider: local" "$SETTINGS_FILE"; then
        error "Error: cloud_provider must be set to 'local' in settings file"
        exit 1
    fi
}

# Step 1b: Setup local code symlink (if --use-local-code is set)
setup_local_code() {
    if [ "$USE_LOCAL_CODE" = false ]; then
        return 0
    fi
    
    info "Setting up local code symlink..."
    
    # When using local code, automatically skip update to avoid git operations
    if [ "$SKIP_UPDATE" = false ]; then
        info "Automatically enabling --skip-update when using local code"
        SKIP_UPDATE=true
    fi
    
    # Extract working_dir and repo URL from settings
    # Handle both quoted and unquoted values, and environment variable substitution
    WORKING_DIR=$(grep "^working_dir:" "$SETTINGS_FILE" | sed -E 's/.*: *"?([^"]*)"?/\1/' | sed "s|\${HOME}|$HOME|g" | sed "s|\${USER}|$USER|g")
    REPO_URL=$(grep "^  url:" "$SETTINGS_FILE" | sed -E 's/.*: *"?([^"]*)"?/\1/')
    
    # Validate we got the values
    if [ -z "$WORKING_DIR" ]; then
        error "Error: Could not extract working_dir from settings file"
        exit 1
    fi
    
    if [ -z "$REPO_URL" ]; then
        error "Error: Could not extract repository URL from settings file"
        exit 1
    fi
    
    # Extract repo name from URL (e.g., "mysticeti" from "https://github.com/asonnino/mysticeti.git")
    REPO_NAME=$(basename "$REPO_URL" .git)
    
    if [ -z "$REPO_NAME" ]; then
        error "Error: Could not extract repository name from URL: $REPO_URL"
        exit 1
    fi
    
    # Create working directory if it doesn't exist
    mkdir -p "$WORKING_DIR"
    
    # Path where orchestrator expects the repo
    REPO_PATH="$WORKING_DIR/$REPO_NAME"
    
    # Remove existing directory/symlink if it exists
    if [ -e "$REPO_PATH" ]; then
        if [ -L "$REPO_PATH" ]; then
            info "Removing existing symlink: $REPO_PATH"
            rm "$REPO_PATH"
        elif [ -d "$REPO_PATH" ]; then
            warning "Directory already exists at $REPO_PATH"
            warning "Backing up to $REPO_PATH.backup and creating symlink"
            mv "$REPO_PATH" "$REPO_PATH.backup"
        fi
    fi
    
    # Create symlink to current project
    info "Creating symlink: $REPO_PATH -> $PROJECT_ROOT"
    ln -s "$PROJECT_ROOT" "$REPO_PATH"
    
    success "Local code symlink created"
    info "Orchestrator will use code from: $PROJECT_ROOT"
    info "Note: The orchestrator will use your current git branch/commit state"
}

# Step 2: Deploy instances (optional)
deploy_instances() {
    if [ "$DEPLOY_INSTANCES" = true ]; then
        info "Deploying $INSTANCES local instances..."
        cd "$PROJECT_ROOT"
        cargo run --bin orchestrator -- testbed deploy --instances "$INSTANCES" || {
            error "Failed to deploy instances"
            exit 1
        }
        success "Instances deployed"
    fi
}

# Step 3: Run benchmark
run_benchmark() {
    cd "$PROJECT_ROOT"
    
    # Build benchmark command
    BENCH_CMD="cargo run --bin orchestrator -- benchmark --committee $COMMITTEE"
    
    # Add loads - use LOADS_ARGS if provided, otherwise use default LOADS
    if [ ${#LOADS_ARGS[@]} -gt 0 ]; then
        LOADS_TO_USE="${LOADS_ARGS[*]}"
        for load in "${LOADS_ARGS[@]}"; do
            BENCH_CMD="$BENCH_CMD --loads $load"
        done
    else
        LOADS_TO_USE="$LOADS"
        BENCH_CMD="$BENCH_CMD --loads $LOADS"
    fi
    
    info "Running benchmark with committee=$COMMITTEE, loads=$LOADS_TO_USE"
    
    # Add skip flags
    if [ "$SKIP_UPDATE" = true ]; then
        BENCH_CMD="$BENCH_CMD --skip-testbed-update"
        info "Skipping testbed update"
    fi
    
    if [ "$SKIP_CONFIG" = true ]; then
        BENCH_CMD="$BENCH_CMD --skip-testbed-configuration"
        info "Skipping testbed configuration"
    fi
    
    # Run the benchmark
    eval "$BENCH_CMD" || {
        error "Benchmark failed"
        exit 1
    }
    
    success "Benchmark completed!"
}

# Main execution
main() {
    info "=== Local Benchmark Setup ==="
    
    # Ensure we're in the right directory structure
    if [ ! -d "$ORCHESTRATOR_DIR" ]; then
        error "Error: orchestrator directory not found at $ORCHESTRATOR_DIR"
        error "Please run this script from the project root"
        exit 1
    fi
    
    # Setup settings
    setup_settings
    
    # Setup local code symlink if requested
    setup_local_code
    
    if [ "$SETUP_ONLY" = true ]; then
        success "Setup complete. Settings file ready at $SETTINGS_FILE"
        if [ "$USE_LOCAL_CODE" = true ]; then
            info "Local code symlink has been created"
        fi
        info "You can now run benchmarks manually or run this script without --setup-only"
        exit 0
    fi
    
    # Deploy instances if requested
    deploy_instances
    
    # Run benchmark
    run_benchmark
    
    info "=== Done ==="
}

# Run main function
main

