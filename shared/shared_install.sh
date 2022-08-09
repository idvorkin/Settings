# to execute stuff from vim
# <range>w !bash

# Setup BREW
mkdir ~/homebrew && curl -L https://github.com/Homebrew/brew/tarball/master | tar xz --strip 1 -C ~/homebrew

# Run the install script.
. ./brew_packages.sh

# load up the git repo's for plugin managers
git clone https://github.com/VundleVim/Vundle.vim.git ~/.vim/bundle/Vundle.vim
git clone https://github.com/tmux-plugins/tpm ~/.tmux/plugins/tpm

# shared git stetup
git config --global user.email "idvorkin@gmail.com"
git config --global user.name "Igor Dvorkin"
git config --global push.default simple

# Personal respoistories
mkdir ~/gits
cd ~/gits
git clone https://github.com/idvorkin/LinqpadSnippets
git clone https://github.com/idvorkin/idvorkin.github.io
git clone https://github.com/idvorkin/jupyter
git clone https://idvorkin@bitbucket.org/idvorkin/igor2.git
popd

# Link to lots of dot files
ln -s -f ~/settings/mac/karabiner.json ~/.config/karabiner/karabiner.json
ln -s -f ~/settings/default_vimrc ~/.vimrc
ln -s -f ~/settings/shared/litecli_config ~/.config/litecli/config
ln -s -f ~/settings/mac/.xvimrc ~/.xvimrc
ln -s -f ~/settings/mac/.inputrc ~/.inputrc
ln -s -f ~/settings/tmuxp ~/.tmuxp
mkdir ~/.hammerspoon
mkdir ~/.ssh
ln -s -f ~/settings/mac/hammerspoon/init.lua ~/.hammerspoon/init.lua
ln -s -f ~/settings/shared/ctags ~/.ctags
ln -s -f ~/settings/shared/.tmux.conf ~/.tmux.conf
ln -s -f ~/settings/shared/.vimperatorrc ~/.vimperatorrc
ln -s -f ~/settings/shared/ipython_config.py  ~/.ipython/ipython_config.py
ln -s -f ~/settings/shared/ipython_config.py  ~/.ipython/profile_default/ipython_config.py
ln -s -f ~/settings/shared/.ideavim ~/.ideavimrc
ln -s -f ~/settings/shared/ssh_config ~/.ssh/config

# Not quite ready to dick iwith it.
# ln -s -f ~/settings/shared/gitconfig ~/.gitconfig

# link git aliases
ln -s -f ~/gits/jupyter ~/ghju
ln -s -f ~/gits/igor2/750words ~/750
ln -s -f ~/gits/igor2/ ~/igor2
ln -s -f ~/gits/idvorkin.github.io ~/blog


#oh my zsh setup - from not fish.
sh -c "$(curl -fsSL https://raw.github.com/robbyrussell/oh-my-zsh/master/tools/install.sh)"


# ctags is not maintained, use universal-ctags instead
brew install --HEAD universal-ctags/universal-ctags/universal-ctags

# /usr/local/bin/ctags -R --langmap=ObjectiveC:.m.h

# copy this into somewhere useful - perhaps ohmyzsh plugin handles.
# eval "$(fasd --init auto)"

# setup useful packages for python
pip3 install tmuxp pipenv pytz glances

# Setup italics term info...
# https://sookocheff.com/post/vim/italics/
tic -o ~/.terminfo tmux.terminfo
tic -o ~/.terminfo tmux-256color.terminfo
tic -o ~/.terminfo xterm-256color.terminfo

# powerline fonts - cyan https://github.com/powerline/fonts
pushd ~/gits
git clone https://github.com/powerline/fonts.git --depth=1
cd fonts
./install.sh

# Remote terminal access
# cat | seashells
# cat | seashells --delay 5 # see url before gone.
pip3 install seashells

# however when looking at directories in WSL that are sourced (like vundle, clone with autcrlf)
# git clone <director> --config core.autocrlf=true

# ensure git can be shared between linux and windows

# git config --global core.autocrlf true
git config --global receive.denyCurrentBranch updateInstead

# Config git-so-fancy
# https://github.com/so-fancy/diff-so-fancy
git config --global core.pager "diff-so-fancy | less --tabs=4 -RFX"

git config --global color.ui true

git config --global color.diff-highlight.oldNormal    "red bold"
git config --global color.diff-highlight.oldHighlight "red bold 52"
git config --global color.diff-highlight.newNormal    "green bold"
git config --global color.diff-highlight.newHighlight "green bold 22"

git config --global color.diff.meta       "yellow"
git config --global color.diff.frag       "magenta bold"
git config --global color.diff.commit     "yellow bold"
git config --global color.diff.old        "red bold"
git config --global color.diff.new        "green bold"
git config --global color.diff.whitespace "red reverse"
git config --global user.email "idvorkin@gmail.com"
git config --global user.name "Igor Dvorkin"
git config --global push.default simple
git config --global alias.co checkout
git config --global alias.com "checkout master"
git config --global alias.fr "pull --rebase"
git config --global alias.br branch
git config --global alias.ci commit
git config --global alias.st status
git config --global alias.logc log master..
git config --global alias.logp "log --pretty=format:'%C(yellow)%h%Cred%d %Creset%s %C(yellow)[%cn] %C(green)(%ar)' --decorate"

# Now I run jekyll in a container -- w00t!

# Set timezone
# sudo timedatectl set-timezone  America/Los_Angeles

