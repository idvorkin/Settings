#!/bin/bash
# Mac-specific setup (non-brew)
# For brew packages, use: ./py/brew_check.py check

# Manually install monaco nerd font
git clone https://github.com/Karmenzind/monaco-nerd-fonts ~/rare_gits/monaco-nerd-fonts

# Fix up key repeats
defaults write NSGlobalDomain ApplePressAndHoldEnabled -bool false

# Disable saving application state on logout
defaults write com.apple.loginwindow TALLogoutSavesState -bool false

# Mac only git setup
git config --global credential.helper osxkeychain
