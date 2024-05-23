
brew install macvim

# run shared install here.

# Install cask
brew install caskroom/cask/brew-cask
# Looks like we need CASK to do things that are findable.
brew install google-chrome  iterm2	skype the-unarchiver
brew install visual-studio-code hammerspoon
brew install db-browser-for-sqlite
brew install finicky
brew install bartender
brew install --cask rectangle
brew install --cask alt-tab
brew install --cask karabiner-elements iterm2 alfred bartender alt-tab hammerspoon  visual-studio-code docker vlc microsoft-edge 1password meetingbar

# Switch between audio input and output
brew install switchaudio-osx
brew install blackhole-2ch

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



# brew cask install osxfuse


# fix up key repeats

defaults write com.visualstudio.code.oss ApplePressAndHoldEnabled -bool false
defaults write com.microsoft.VSCodeInsiders ApplePressAndHoldEnabled -bool false
defaults write com.microsoft.VSCode ApplePressAndHoldEnabled -bool false
defaults write -app /Applications/Visual\ Studio\ Code.app ApplePressAndHoldEnabled -bool false


brew linkapps macvim

# Mac only git setup
git config --global credential.helper osxkeychain
git config --global core.editor /usr/local/bin/vim


# setup xcode
git clone https://github.com/XVimProject/XVim.git  ~/gits/XVim
cd ~/gits/XVim
#make

# window manager -- pretty cool

# Install better snap tool
