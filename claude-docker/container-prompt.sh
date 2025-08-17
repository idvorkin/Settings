#!/bin/bash
# Container prompt setup script
# This script configures the shell prompt to include the Docker container name

# Check if we're in a Docker container with a name set
if [ -n "$DOCKER_CONTAINER_NAME" ]; then
    # For bash
    if [ -n "$BASH_VERSION" ]; then
        export PS1="🐳[$DOCKER_CONTAINER_NAME] $PS1"
    fi
    
    # For zsh
    if [ -n "$ZSH_VERSION" ]; then
        # Simple prompt prefix for container name
        export PS1="🐳[$DOCKER_CONTAINER_NAME] $PS1"
    fi
    
    # Export for child processes
    export DOCKER_CONTAINER_DISPLAY="🐳 $DOCKER_CONTAINER_NAME"
fi

# Configure git to use GH_TOKEN for GitHub authentication
if [ -n "$GH_TOKEN" ]; then
    # Just configure git to embed the token in GitHub URLs
    git config --global url."https://idvorkin-ai-tools:${GH_TOKEN}@github.com/".insteadOf "https://github.com/"
    echo "Git configured to use GH_TOKEN for GitHub authentication"
fi

# Copy Claude credentials if mounted
if [ -f "/ro_host/.claude/.credentials.json" ]; then
    mkdir -p ~/.claude
    cp /ro_host/.claude/.credentials.json ~/.claude/.credentials.json
    chmod 600 ~/.claude/.credentials.json
    echo "Claude credentials copied from host"
fi