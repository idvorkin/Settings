# to execute stuff from vim
# <range>w !bash

# Setup BREW
mkdir ~/homebrew && curl -L https://github.com/Homebrew/brew/tarball/master | tar xz --strip 1 -C ~/homebrew

# Run the install script.
. ./brew_packages.sh


. ~/settings/bootrap.sh

# Some private dot files
mkdir ~/.ssh
ln -s -f ~/settings/shared/ssh_config ~/.ssh/config

# Not quite ready to dick iwith it.
# ln -s -f ~/settings/shared/gitconfig ~/.gitconfig

# link git aliases
ln -s -f ~/gits/jupyter ~/ghju
ln -s -f ~/gits/igor2/750words ~/750
ln -s -f ~/gits/igor2/ ~/igor2
ln -s -f ~/gits/idvorkin.github.io ~/blog




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


# Now I run jekyll in a container -- w00t!

# Set timezone
# sudo timedatectl set-timezone  America/Los_Angeles

