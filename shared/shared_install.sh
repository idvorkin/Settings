# looks like brew is just for command line stuff.
brew install brew-cask cask emacs zsh libevent openssl tmux	ag  git wget ncdu graphviz htop

# Make sure install vim with python for denite
brew install --with-python3 vim


# shared git stetup
git config --global user.email "idvorkin@gmail.com"
git config --global user.name "Igor Dvorkin"
git config --global push.default simple

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

#oh my zsh setup - from not fish.
sh -c "$(curl -fsSL https://raw.github.com/robbyrussell/oh-my-zsh/master/tools/install.sh)"

# Update .zshrc with set -o vi 

# Add ctags for vim
brew install ctags --HEAD
# /usr/local/bin/ctags -R --langmap=ObjectiveC:.m.h
