#!/bin/bash
# First install essential development tools
echo "Installing essential development tools..."
brew install git tmux zsh neovim

# Install Rust and cargo
echo "Installing Rust and cargo..."
curl https://sh.rustup.rs -sSf | sh -s -- -y
source "$HOME/.cargo/env"

# Install packages via cargo for better cross-platform compatibility
echo "Installing Rust packages via cargo..."
cargo install zoxide bat eza mcfly dua procs htop starship yazi television bottom rg

# Define the remaining brew packages
export brew_packages="\
fd \
lazygit \
fzf \
ag \
yt-dlp \
gdu \
yq \
cpulimit \
docker \
png-quant \
docker-compose \
lua-language-server \
diff-so-fancy \
fx \
git-extras \
git-delta \
just \
imagemagick \
python3 \
asciinema \
asciiquarium \
ffmpeg \
pre-commit \
fselect \
btop \
lazydocker \
markdownlint-cli \
mosh \
ncdu \
nmap \
npm \
openssl \
optipng \
pipenv \
pgcli \
pup \
s3cmd \
wget \
w3m \
xh \
zip \
difftastic \
pngpaste \
cmatrix"

# HTTP downloaders
brew_packages="$brew_packages curlie" # Curl with a UI
brew_packages="$brew_packages httpie" # Another downloader, cli is http

# Data editors
brew_packages="$brew_packages dasel" # jq replacement
brew_packages="$brew_packages jq"
brew_packages="$brew_packages duf"

# System info & monitoring
brew_packages="$brew_packages fastfetch" # Show system info (faster alternative to neofetch)

# Terminal-based image viewers
brew_packages="$brew_packages viu"
brew_packages="$brew_packages timg"

# Source code tools
brew_packages="$brew_packages gh" # official github cli
brew_packages="$brew_packages cloc" # count lines of code
brew_packages="$brew_packages tig" # git history viewer

# Document conversion
brew_packages="$brew_packages pandoc" # Convert between file formats

# Markdown viewers
brew_packages="$brew_packages glow"
brew_packages="$brew_packages grip" # GitHub Markdown preview tool

# Network tools
brew_packages="$brew_packages autossh"  # auto reconnect - like MOSH, but works better w/NVIM

# Shell enhancement
brew_packages="$brew_packages atuin"  # shell history, like mcfly, but slower

# Python package management
brew_packages="$brew_packages pipx uv"  # uv is a replacement for pip, and WAAAAAY faster, especially useful for pipx replacement

# Media players
brew_packages="$brew_packages mpv" # Great video player, mostly works from CLI

# Fun visualization tools
brew_packages="$brew_packages cmatrix" # Matrix-like terminal animation

echo "Installing remaining brew packages..."
echo $brew_packages

# Fetch packages in parallel for speed
echo $brew_packages | xargs -n1 --max-procs=8 brew fetch
# Install packages in series
echo $brew_packages | xargs -n1 brew install

# Get latest version of mosh
brew install mosh --head

# Correct version of tags
brew install --HEAD universal-ctags/universal-ctags/universal-ctags

# GitHub extensions
gh extension install github/gh-copilot

# Add some npm packages
npm install --global fkill-cli

# Install pipx tools
echo "Setting up Python tools..."
pipx install pipxu # pipx upgrade tool
pipxu install aider-chat # Code Writing helper
pipxu install ruff # Fast Python linter
pipxu install httpx # HTTP client with CLI
pipxu install pre-commit # Git pre-commit hooks framework
pipxu install jupyterlab # Jupyter notebook interface
pipxu install rich-cli # Terminal formatting tool

# Cloud tools
echo "Installing cloud tools..."
brew install --cask google-cloud-sdk

echo "Setup completed successfully!"

# Note: Other Python CLI tools that can be installed via pipx if needed:
# - black: Code formatter
# - mypy: Static type checker
# - poetry: Python package manager
# - uvicorn: ASGI server
# - pudb: Console-based visual debugger
