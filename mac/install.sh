#!/bin/bash
# Mac-specific setup (non-brew)
# For brew packages, use: ./py/brew_check.py check

# Manually install monaco nerd font
git clone https://github.com/Karmenzind/monaco-nerd-fonts ~/rare_gits/monaco-nerd-fonts

# Fix up key repeats
defaults write NSGlobalDomain ApplePressAndHoldEnabled -bool false

# Disable saving application state on logout
defaults write com.apple.loginwindow TALLogoutSavesState -bool false

# Dock: auto-hide, small icons that magnify on hover
defaults write com.apple.dock autohide -bool true
defaults write com.apple.dock tilesize -int 36
defaults write com.apple.dock magnification -bool true
defaults write com.apple.dock largesize -int 80
killall Dock

# Mac only git setup
git config --global credential.helper osxkeychain
