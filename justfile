# Justfile for running tests
default:
    @just --list

# Fast tests - called by pre-commit
fast-test:
    @nvim -l nvim/tests/minit.lua

# All tests
test:
    @nvim -l nvim/tests/minit.lua

# Install locally and globally
install:
    @cd py && uv venv
    @cd py && . .venv/bin/activate
    @cd py && uv pip install --upgrade --editable .

global-install: install
    @cd py && uv tool install --force  --editable .

# Install rust tmux helper (skips rebuild if already up-to-date)
rinstall:
    #!/usr/bin/env bash
    set -euo pipefail
    cd rust/tmux_helper
    current_hash=$(git rev-parse --short HEAD)
    installed_version=$(rmux_helper --version 2>/dev/null || echo "")
    if [[ "$installed_version" == *"$current_hash"* ]]; then
        echo "rmux_helper already up-to-date ($current_hash)"
    else
        echo "Building and installing rmux_helper ($current_hash)..."
        cargo install --path . --force
    fi

