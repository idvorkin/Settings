ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)" 
# looks like brew is just for command line stuff.
brew install brew-cask	cask		emacs		zsh		libevent	openssl		tmux	ag  git macvim wget ncdu graphviz

# Make sure install vim with python for denite
brew install --with-python3 vim

# Install cask
brew install caskroom/cask/brew-cask
# Looks like we need CASK to do things that are findable. 
brew cask install google-chrome  iterm2	skype spectacle sourcetree virtualbox vagrant macdown the-unarchiver anaconda
brew cask install visual-studio-code hammerspoon


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
git config --global core.editor /usr/local/bin/vim

# in seil map caps -> F19 (Keycode: 80) 
ln -s -f ~/settings/private.xml ~/Library/Application\ Support/karabiner/private.xml
ln -s -f ~/settings/mac/karabiner.json ~/.config/karabiner/karabiner.json
ln -s -f ~/settings/default_vimrc ~/.vimrc 
ln -s -f ~/settings/mac/.xvimrc ~/.xvimrc 
ln -s -f ~/settings/mac/.inputrc ~/.inputrc 
mkdir ~/.hammerspoon
ln -s -f ~/settings/mac/hammerspoon/init.lua ~/.hammerspoon/init.lua
ln -s -f ~/settings/shared/ctags ~/.ctagsrc 
ln -s -f ~/settings/shared/.tmux.conf ~/.tmux.conf
ln -s -f ~/settings/shared/.vimperatorrc ~/.vimperatorrc
ln -s -f ~/settings/shared/.ideavim ~/.ideavimrc

# Setup vundle
git clone https://github.com/gmarik/vundle.git ~/vimfiles/bundle/vundle
# setup xcode
git clone https://github.com/XVimProject/XVim.git  ~/gits/XVim
cd ~/gits/XVim
#make

#oh my zsh setup - from not fish.
sh -c "$(curl -fsSL https://raw.github.com/robbyrussell/oh-my-zsh/master/tools/install.sh)"

# Update .zshrc with set -o vi 

# Add ctags for vim
brew install ctags --HEAD
# /usr/local/bin/ctags -R --langmap=ObjectiveC:.m.h

# Jekyll
# gem install sudo gem install -n /usr/local/bin/ jekyll jekyll-paginate

#Alacatraz
curl -fsSL https://raw.github.com/supermarin/Alcatraz/master/Scripts/install.sh | sh

# Git setup for newer git
