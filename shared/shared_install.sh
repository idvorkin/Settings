# goto http://linuxbrew.sh/
# looks like brew is just for command line stuff.
brew install cask zsh libevent openssl tmux ag  git wget ncdu graphviz htop python3 ranger diff-so-fancy fzf bat fd ruby aws-shell jq ag npm mosh exa asciinema rg httpie

# packages I want that don't exist
# brew install saws svg-term

# currently broken on some devices.
brew install azure-cli cmatrix iftop

# ranger = File Explorer
# grv - get repository viewer
# grv need to unalias grv in zsh


# to execute it
# <range>w !bash

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
mkdir ~/.ssh
ln -s -f ~/settings/mac/hammerspoon/init.lua ~/.hammerspoon/init.lua
ln -s -f ~/settings/shared/ctags ~/.ctagsrc
ln -s -f ~/settings/shared/.tmux.conf ~/.tmux.conf
ln -s -f ~/settings/shared/.vimperatorrc ~/.vimperatorrc
ln -s -f ~/settings/shared/.ideavim ~/.ideavimrc
ln -s -f ~/settings/shared/ssh_config ~/.ssh/ssh_config

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
pip3 install --user tmuxp pipenv

# Add links to linux brew
# Play - tmuxp alias ~/Library/Python/3.6/bin/tmuxp
echo -n 'export PATH=/home/linuxbrew/.linuxbrew/bin/:$PATH' >> ~/.zshrc


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
git config --global core.autocrlf true
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

# share credentila manager between WSL and windows desktop
git config --global credential.helper "/mnt/c/Program\ Files/Git/mingw64/libexec/git-core/git-credential-manager.exe"


# load up the git repo's for plugins
git clone https://github.com/VundleVim/Vundle.vim.git ~/.vim/bundle/Vundle.vim
git clone https://github.com/tmux-plugins/tpm ~/.tmux/plugins/tpm

# Here's stuff for dotnet.
wget https://packages.microsoft.com/config/ubuntu/18.04/packages-microsoft-prod.deb
sudo dpkg -i packages-microsoft-prod.deb
sudo add-apt-repository universe
sudo apt-get install apt-transport-https
sudo apt-get update
sudo apt-get install dotnet-sdk-2.2

# and the way to do it on AMI since it can't build the dependancies.

wget https://download.microsoft.com/download/5/F/0/5F0362BD-7D0A-4A9D-9BF9-022C6B15B04D/dotnet-runtime-2.0.0-linux-x64.tar.gz
mkdir -p $HOME/dotnet && tar zxf dotnet-runtime-2.0.0-linux-x64.tar.gz -C $HOME/dotnet
export PATH=$PATH:$HOME/dotnet


#here's stuff for az-cli install
# This is currently broken -- wait a bit and hope it gets fixed. For now use cloud desktop.
sudo apt-get install apt-transport-https lsb-release software-properties-common -y
AZ_REPO=$(lsb_release -cs)
echo "deb [arch=amd64] https://packages.microsoft.com/repos/azure-cli/ $AZ_REPO main" | \
        sudo tee /etc/apt/sources.list.d/azure-cli.list

# Add pipenv
pip3 install pipenv

# Cool command to run through all directories and pull them
alias 'gfrall= for git_directory in * ; echo $git_directory && git -C $git_directory fr'

# when locales get screwed up
export LANGUAGE=en_US.UTF-8
export LANG=en_US.UTF-8
export LC_ALL=en_US.UTF-8

