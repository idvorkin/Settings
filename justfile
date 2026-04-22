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
    @cd py && uv venv --allow-existing
    @cd py && uv pip install --upgrade --editable .

global-install: install
    @cd py && uv tool install --force  --editable .

# Wire up git hooks and build all tools
setup:
    ln -sf ../../shared/settings-git-hooks/post-merge .git/hooks/post-merge
    just global-install
    just rinstall

# Install rust tmux helper (skips rebuild if already up-to-date)
rinstall:
    #!/usr/bin/env bash
    set -euo pipefail
    cd rust/tmux_helper
    current_hash=$(git rev-parse --short HEAD)
    dirty=$(git status --porcelain -- . | head -n1)
    installed_version=$(rmux_helper --version 2>/dev/null || echo "")
    if [[ -z "$dirty" && "$installed_version" == *"$current_hash"* ]]; then
        echo "rmux_helper already up-to-date ($current_hash)"
    else
        echo "Building and installing rmux_helper ($current_hash${dirty:+ +dirty})..."
        cargo install --path . --force
    fi

