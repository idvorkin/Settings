
brew install macvim

# run shared install here.

# Install cask
brew install caskroom/cask/brew-cask
# Looks like we need CASK to do things that are findable.
brew install --cask google-chrome  iterm2	skype the-unarchiver
brew install --cask visual-studio-code hammerspoon  vlc
brew install --cask db-browser-for-sqlite
brew install --cask bartender
brew install --cask finicky

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
