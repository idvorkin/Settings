ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)" 
# looks like brew is just for command line stuff.
brew install brew-cask	cask		emacs		zsh		libevent	openssl		tmux	vim ag  git macvim wget ncdu
# Install cask
brew install caskroom/cask/brew-cask
# Looks like we need CASK to do things that are findable. 
brew cask install google-chrome  iterm2	      karabiner	     seil	    skype spectacle sourcetree virtualbox vagrant macdown the-unarchiver


# window manager -- pretty cool 
# https://www.spectacleapp.com/
brew cask install spectacle

brew linkapps macvim
# shared git stetup

git config --global user.email "idvorkin@gmail.com"
git config --global user.name "Igor Dvorkin"
git config --global push.default simple
# Mac only git setup
git config --global credential.helper osxkeychain
git config --global core.editor /usr/bin/vim

# in seil map caps -> F19 (Keycode: 80) 
ln -s -f ~/settings/private.xml ~/Library/Application\ Support/karabiner/private.xml
ln -s -f ~/settings/default_vimrc ~/.vimrc 
ln -s -f ~/settings/mac/.xvimrc ~/.xvimrc 

# Setup vundle
git clone https://github.com/gmarik/vundle.git ~/vimfiles/bundle/vundle
# setup xcode
git clone https://github.com/XVimProject/XVim.git  ~/gits/XVim
cd ~/gits/XVim
make

#oh my zsh setup - from not fish.
sh -c "$(curl -fsSL https://raw.github.com/robbyrussell/oh-my-zsh/master/tools/install.sh)"

# Update .zshrc with set -o vi 

# Add ctags for vim
brew install ctags --HEAD
# /usr/local/bin/ctags -R --langmap=ObjectiveC:.m.h

#Alacatraz
curl -fsSL https://raw.github.com/supermarin/Alcatraz/master/Scripts/install.sh | sh

# Git setup for newer git
