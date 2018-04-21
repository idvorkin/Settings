# looks like brew is just for command line stuff.
brew install cask zsh libevent openssl tmux	ag  git wget ncdu graphviz htop python3

# Make sure install vim with python for denite
brew install --with-python3 vim

# brew install fasd

# shared git stetup
git config --global user.email "idvorkin@gmail.com"
git config --global user.name "Igor Dvorkin"
git config --global push.default simple

ln -s -f ~/settings/mac/karabiner.json ~/.config/karabiner/karabiner.json
ln -s -f ~/settings/default_vimrc ~/.vimrc 
ln -s -f ~/settings/mac/.xvimrc ~/.xvimrc 
ln -s -f ~/settings/mac/.inputrc ~/.inputrc 
ln -s -f ~/settings/tmuxp ~/.tmuxp
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

# setup fasd
pushd ~/gits

git clone https://github.com/clvv/fasd 
cd fasd
sudo make install

# copy this into somewhere useful
# eval "$(fasd --init auto)"



# setup more tmux plugins (not sure that they are useful.
git clone https://github.com/tmux-plugins/tpm ~/.tmux/plugins/tpm


# setup tmuxp
pip3 install --user tmuxp
# Play - tmuxp alias ~/Library/Python/3.6/bin/tmuxp


# Setup italics term info...
# https://sookocheff.com/post/vim/italics/
tic -o ~/.terminfo tmux.terminfo
tic -o ~/.terminfo tmux-256color.terminfo
tic -o ~/.terminfo xterm-256color.terminfo
