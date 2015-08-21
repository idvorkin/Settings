ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)" 
# looks like brew is just for command line stuff.
brew install brew-cask	cask		emacs		zsh		libevent	openssl		tmux	vim ag 
# Looks like we need CASK to do things that are findable.
brew cask install google-chrome  iterm2	      karabiner	     seil	    skype macvim

# shared git stetup

git config --global user.email "idvorkin@gmail.com"
git config --global user.name "Igor Dvorkin"
git config --global push.default simple

# in seil map caps -> F19 (Keycode: 80) 
ln ~/settings/private.xml ~/Library/Application\ Support/karabiner/private.xml
ln ~/settings/default_vimrc ~/.vimrc 
ln ~/settings/mac/.xvimrc ~/.xvimrc 

# Setup vundle
git clone https://github.com/gmarik/vundle.git ~/vimfiles/bundle/vundle
# setup xcode
cd ~/gits
git clone https://github.com/XVimProject/XVim.git 
cd XVim
make

cd ~/gits

#oh my zsh setup - from not fish.
sh -c "$(curl -fsSL https://raw.github.com/robbyrussell/oh-my-zsh/master/tools/install.sh)"

# Update .zshrc with set -o vi 

# Add ctags for vim
brew install ctags --HEAD
# /usr/local/bin/ctags -R --langmap=ObjectiveC:.m.h

#Alacatraz
curl -fsSL https://raw.github.com/supermarin/Alcatraz/master/Scripts/install.sh | sh
