
brew install macvim

# run shared install here.

# Install cask
brew install caskroom/cask/brew-cask
# Looks like we need CASK to do things that are findable.
brew cask install google-chrome  iterm2	skype spectacle sourcetree virtualbox vagrant macdown the-unarchiver anaconda
brew cask install visual-studio-code hammerspoon sourcetree
brew cask install osxfuse
brew cask install sshfs vlc



brew linkapps macvim

# Mac only git setup
git config --global credential.helper osxkeychain
git config --global core.editor /usr/local/bin/vim


# setup xcode
git clone https://github.com/XVimProject/XVim.git  ~/gits/XVim
cd ~/gits/XVim
#make


# window manager -- pretty cool
# https://www.spectacleapp.com/
brew cask install spectacle
