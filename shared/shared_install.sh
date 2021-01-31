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
git clone https://github.com/idvorkin/onom
git clone https://github.com/idvorkin/Vim-Keybindings-For-Onenote
git clone https://github.com/idvorkin/LinqpadSnippets
git clone https://github.com/idvorkin/linqpadDataExplore
git clone https://github.com/idvorkin/idvorkin.github.io
git clone https://github.com/idvorkin/jupyter
git clone https://idvorkin@bitbucket.org/idvorkin/igor2.git

# Link to lots of dot files
ln -s -f ~/settings/mac/karabiner.json ~/.config/karabiner/karabiner.json
ln -s -f ~/settings/default_vimrc ~/.vimrc
ln -s -f ~/settings/mac/.xvimrc ~/.xvimrc
ln -s -f ~/settings/mac/.inputrc ~/.inputrc
ln -s -f ~/settings/tmuxp ~/.tmuxp
mkdir ~/.hammerspoon
mkdir ~/.ssh
ln -s -f ~/settings/mac/hammerspoon/init.lua ~/.hammerspoon/init.lua
ln -s -f ~/settings/shared/ctags ~/.ctags
ln -s -f ~/settings/shared/.tmux.conf ~/.tmux.conf
ln -s -f ~/settings/shared/.vimperatorrc ~/.vimperatorrc
ln -s -f ~/settings/shared/ipython_config.py  ~/.ipython/profile_default/ipython_config.py
ln -s -f ~/settings/shared/.ideavim ~/.ideavimrc
ln -s -f ~/settings/shared/ssh_config ~/.ssh/config

# Not quite ready to dick iwith it.
# ln -s -f ~/settings/shared/gitconfig ~/.gitconfig

# link git aliases
ln -s -f ~/gits/techdiary ~/ghtd
ln -s -f ~/gits/jupyter ~/ghju
ln -s -f ~/gits/igor2/750words ~/750
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

# share credentila manager between WSL and windows desktop
# git config --global credential.helper "/mnt/c/Program\ Files/Git/mingw64/libexec/git-core/git-credential-manager.exe"
# https://git-scm.com/book/en/v2/Git-Tools-Credential-Storage
# git config --global credential.helper 'store --file ~/.my-credentials'



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

# Setup Jekyll
# Rant why so god damn hard to build jekyll and nokogiri
# be sure to set path to brew ruby in your zshrc
# export PATH=/usr/local/opt/ruby/bin:$PATH
gem install bundle
brew install rbenv
gem install --user-install bundler jekyll
cd

# Set timezone
# sudo timedatectl set-timezone  America/Los_Angeles

# when locales get screwed up - haven't needed in a while.
# export LANGUAGE=en_US.UTF-8
# xport LANG=en_US.UTF-8
# export port LC_ALL=en_US.UTF-8
# export PATH=/usr/local/opt/ruby/bin:$PATH
