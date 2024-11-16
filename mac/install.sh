
# run shared install here.

# Looks like we need CASK to do things that are findable.
brew install --cask google-chrome  iterm2	the-unarchiver
brew install --cask db-browser-for-sqlite

brew install --cask finicky # URL handler

# Window managers - I've tried:
# BetterSnaTool, Rectangle, now i use yabai
# brew install --cask rectangle

brew install --cask alt-tab # windows like alt-tab handling
brew install jordanbaird-ice # Collapse menu bar (replaces bartender)


# window managers
brew install --cask nikitabobko/tap/aerospace



# Janky borders

brew tap FelixKratz/formulae
brew install borders
brew install koekeishiya/formulae/yabai

brew install --cask karabiner-elements iterm2 alfred visual-studio-code docker microsoft-edge 1password meetingbar

# Switch between audio input and output - these mess stuff up, get rid of 'em.
# brew install switchaudio-osx
# brew install blackhole-2ch

# Display resolutoin shifting
brew install jakehilborn/jakehilborn/displayplacer

# Video player, better then vlc
brew install iina

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

# Mac only git setup
git config --global credential.helper osxkeychain


# setup xcode
git clone https://github.com/XVimProject/XVim.git  ~/gits/XVim
cd ~/gits/XVim
#make
