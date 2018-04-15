# Ruby
ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)" 

# Linux -- 

# sh -c "$(curl -fsSL https://raw.githubusercontent.com/Linuxbrew/install/master/install.sh)"

brew install macvim 

# run shared install here.

# Install cask
brew install caskroom/cask/brew-cask
# Looks like we need CASK to do things that are findable. 
brew cask install google-chrome  iterm2	skype spectacle sourcetree virtualbox vagrant macdown the-unarchiver anaconda
brew cask install visual-studio-code hammerspoon sourcetree 
brew cask install osxfuse
brew cask install sshfs vlc


# window manager -- pretty cool 
# https://www.spectacleapp.com/
brew cask install spectacle

brew linkapps macvim

# Mac only git setup
git config --global credential.helper osxkeychain
git config --global core.editor /usr/local/bin/vim


# Setup vundle
git clone https://github.com/gmarik/vundle.git ~/vimfiles/bundle/vundle
# setup xcode
git clone https://github.com/XVimProject/XVim.git  ~/gits/XVim
cd ~/gits/XVim
#make


# Jekyll
# gem install sudo gem install -n /usr/local/bin/ jekyll jekyll-paginate

#Alacatraz
curl -fsSL https://raw.github.com/supermarin/Alcatraz/master/Scripts/install.sh | sh
