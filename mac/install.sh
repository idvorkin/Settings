# run shared install here.

# Looks like we need CASK to do things that are findable.
brew install --cask google-chrome  iterm2	the-unarchiver

brew install --cask finicky # URL handler

# Window managers - I've tried:
# BetterSnaTool, Rectangle, now i use yabai
# brew install --cask rectangle

brew install --cask alt-tab # windows like alt-tab handling
brew install jordanbaird-ice # Collapse menu bar (replaces bartender)


# window managers - I find aerospace isn't stable yet
# brew install --cask nikitabobko/tap/aerospace


# I like this highlight active window   with borders
brew tap FelixKratz/formulae
brew install borders

brew install koekeishiya/formulae/yabai

brew install --cask karabiner-elements
brew install --cask iterm2
brew install --cask alfred
# brew install --cask docker # Replaced by Orbstack
brew install --cask microsoft-edge
brew install --cask 1password
brew install --cask tailscale
brew install --cask meetingbar
brew install --cask cursor
brew install --cask capcut
brew install --cask signal
brew install --cask kindavim

# Switch between audio input and output - these mess stuff up, get rid of 'em.
# brew install switchaudio-osx
# brew install blackhole-2ch

# Display resolutions shifting
# Messes stuff up, ignore for now.
# brew install jakehilborn/jakehilborn/displayplacer
brew install --cask betterdisplay

# Video player, better than vlc
# Also check out mpv
brew install iina

# Video editing tool
brew install --cask losslesscut

# QuickLook plugin for video thumbnails and previews (MKV, WebM, etc.)
# https://github.com/Marginal/QLVideo
brew install --cask qlvideo

# Install fonts
brew tap homebrew/cask-fonts
# Add a specific font
brew install --cask font-hack-nerd-font

# Manulaly install the font
git clone https://github.com/Karmenzind/monaco-nerd-fonts ~/rare_gits/monaco-nerd-fonts

# Show the keys being pressed
brew install --cask keycastr

# brew cask install osxfuse


# fix up key repeats

defaults write NSGlobalDomain ApplePressAndHoldEnabled -bool false

# Disable saving application state on logout
defaults write com.apple.loginwindow TALLogoutSavesState -bool false

# Mac only git setup
git config --global credential.helper osxkeychain

