#!/bin/bash

# Exit on error
set -e

# Check if GITHUB_PERSONAL_ACCESS_TOKEN is set in the environment
if [ -z "$GITHUB_PERSONAL_ACCESS_TOKEN" ]; then
    echo "Error: GITHUB_PERSONAL_ACCESS_TOKEN environment variable is not set" >&2
    exit 1
fi

# Run the GitHub MCP server
docker run -i --rm -e GITHUB_PERSONAL_ACCESS_TOKEN="$GITHUB_PERSONAL_ACCESS_TOKEN" ghcr.io/github/github-mcp-server "$@" 