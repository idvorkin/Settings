#!/bin/bash

# Exit on error
set -e

# NOTE: This script hardcodes the GITHUB_PERSONAL_ACCESS_TOKEN environment variable
# because some MCP clients don't properly pass environment variables to the server.
# This is a workaround for that limitation - ideally the MCP client would handle
# environment variable propagation correctly.

# Check if GITHUB_PERSONAL_ACCESS_TOKEN is set in the environment
# If not, try to read it from secretBox.json using jq
if [ -z "$GITHUB_PERSONAL_ACCESS_TOKEN" ]; then
    # Check if secretBox.json exists
    SECRET_FILE="$HOME/gits/igor2/secretBox.json"
    if [ -f "$SECRET_FILE" ]; then
        GITHUB_PERSONAL_ACCESS_TOKEN=$(jq -r .GITHUB_PERSONAL_ACCESS_TOKEN "$SECRET_FILE")
        if [ "$GITHUB_PERSONAL_ACCESS_TOKEN" = "null" ] || [ -z "$GITHUB_PERSONAL_ACCESS_TOKEN" ]; then
            echo "Error: GITHUB_PERSONAL_ACCESS_TOKEN not found in $SECRET_FILE" >&2
            exit 1
        fi
    else
        echo "Error: GITHUB_PERSONAL_ACCESS_TOKEN environment variable is not set and $SECRET_FILE not found" >&2
        exit 1
    fi
fi

# Run the GitHub MCP server
docker run -i --rm -e GITHUB_PERSONAL_ACCESS_TOKEN="$GITHUB_PERSONAL_ACCESS_TOKEN" ghcr.io/github/github-mcp-server "$@" 