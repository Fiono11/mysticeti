#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# SPDX-License-Identifier: Apache-2.0
#
# Script to run benchmarks locally without cloud instances.
# This automates the setup described in crates/orchestrator/LOCAL_SETUP.md

set -e

RED='\033[1;31m'
GREEN='\033[1;32m'
BLUE='\033[1;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
COMMITTEE=4
LOADS="200"
INSTANCES=""
SKIP_TESTBED_UPDATE=false
SKIP_TESTBED_CONFIG=false
DEPLOY_INSTANCES=false

# Script directory and orchestrator assets directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ORCHESTRATOR_ASSETS="$PROJECT_ROOT/crates/orchestrator/assets"
SETTINGS_LOCAL="$ORCHESTRATOR_ASSETS/settings-local.yml"
SETTINGS_FILE="$ORCHESTRATOR_ASSETS/settings.yml"

# Print usage information
usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Run benchmarks locally on your machine without cloud instances.

OPTIONS:
    -c, --committee NUM          Number of committee nodes (default: 4)
    -l, --loads LOADS            Comma-separated list of transaction loads in tx/s (default: 200)
    -i, --instances NUM          Number of instances to deploy (optional, auto-created if not specified)
    -d, --deploy                 Deploy instances before running benchmark
    -u, --skip-update            Skip testbed update step
    -k, --skip-config            Skip testbed configuration step
    -h, --help                   Show this help message

EXAMPLES:
    # Quick test with default settings
    $0

    # Run with 2 nodes and 100 tx/s load, skip update
    $0 -c 2 -l 100 -u

    # Deploy 4 instances first, then run benchmark
    $0 -d -i 4 -c 4 -l 200

    # Run multiple loads
    $0 -c 4 -l "100,200,500"

EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -c|--committee)
            COMMITTEE="$2"
            shift 2
            ;;
        -l|--loads)
            LOADS="$2"
            shift 2
            ;;
        -i|--instances)
            INSTANCES="$2"
            shift 2
            ;;
        -d|--deploy)
            DEPLOY_INSTANCES=true
            shift
            ;;
        -u|--skip-update)
            SKIP_TESTBED_UPDATE=true
            shift
            ;;
        -k|--skip-config)
            SKIP_TESTBED_CONFIG=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

# Step 1: Setup local settings
echo -e "${BLUE}Setting up local configuration...${NC}"
if [[ ! -f "$SETTINGS_LOCAL" ]]; then
    echo -e "${RED}Error: settings-local.yml not found at $SETTINGS_LOCAL${NC}"
    exit 1
fi

if [[ -f "$SETTINGS_FILE" ]]; then
    echo -e "${YELLOW}Warning: settings.yml already exists. Backing up to settings.yml.backup${NC}"
    cp "$SETTINGS_FILE" "$SETTINGS_FILE.backup"
fi

cp "$SETTINGS_LOCAL" "$SETTINGS_FILE"
echo -e "${GREEN}✓ Local settings configured${NC}"

# Step 2: Deploy instances if requested
if [[ "$DEPLOY_INSTANCES" == true ]]; then
    INSTANCES_ARG="$INSTANCES"
    if [[ -z "$INSTANCES_ARG" ]]; then
        INSTANCES_ARG="$COMMITTEE"
        echo -e "${YELLOW}No instance count specified, using committee size: $INSTANCES_ARG${NC}"
    fi
    
    echo -e "${BLUE}Deploying $INSTANCES_ARG local instances...${NC}"
    cd "$PROJECT_ROOT"
    cargo run --bin orchestrator -- testbed deploy --instances "$INSTANCES_ARG" || {
        echo -e "${RED}Failed to deploy instances${NC}"
        exit 1
    }
    echo -e "${GREEN}✓ Instances deployed${NC}"
fi

# Step 3: Build benchmark command
echo -e "${BLUE}Preparing benchmark command...${NC}"
BENCHMARK_CMD="cargo run --bin orchestrator -- benchmark --committee $COMMITTEE --loads $LOADS"

if [[ "$SKIP_TESTBED_UPDATE" == true ]]; then
    BENCHMARK_CMD="$BENCHMARK_CMD --skip-testbed-update"
fi

if [[ "$SKIP_TESTBED_CONFIG" == true ]]; then
    BENCHMARK_CMD="$BENCHMARK_CMD --skip-testbed-configuration"
fi

# Step 4: Run benchmark
echo -e "${BLUE}Running benchmark with committee=$COMMITTEE, loads=$LOADS${NC}"
echo -e "${YELLOW}Command: $BENCHMARK_CMD${NC}"
echo ""

cd "$PROJECT_ROOT"
eval "$BENCHMARK_CMD" || {
    echo -e "${RED}Benchmark failed${NC}"
    exit 1
}

echo ""
echo -e "${GREEN}✓ Benchmark completed successfully!${NC}"
