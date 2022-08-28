
export brew_packages="\
zoxide \
exa \
jq \
mcfly \
fzf \
bat \
duf \
ag \
fd \
zsh \
tmux \
git \
tig \
docui \
dua-cli \
ranger \
cask \
fasd \
docker \
docker-compose \
diff-so-fancy \
git-extras \
git-delta \
python3 \
asciinema \
asciiquarium \
aws-shell \
ffmpeg \
fselect \
glances \
gotop \
grip \
gdu \
htop \
hub \
httpie \
imagemagick \
jq \
jless \
libevent \
litecli \
lazydocker \
markdownlint-cli \
mosh \
ncdu \
neofetch \
npm \
openssl \
optipng \
pipenv \
pgcli \
pup \
rg \
ruby@2.7 \
s3cmd \
wget \
w3m \
docker \
gdu \
curlie \
xh \
procs \
yasm \
zip \
yarn \
rust \
rbenv \
yarn "

# Notes, rust is super slow to compile, so putting that packages last

# https://unix.stackexchange.com/questions/7558/execute-a-command-once-per-line-of-piped-input
# Ahh magic. xargs takes it's input execute command once per line
echo $brew_packages | xargs -n1 brew install

# to execute things from VIM
# <range>w !bash

#  Get latest version of mosh
brew install mosh --head

# currently broken on some devices.
brew install azure-cli cmatrix iftop


# ranger = File Explorer
# grv - get repository viewer
# grv need to unalias grv in zsh


# Correct version of tags
brew install --HEAD universal-ctags/universal-ctags/universal-ctags

# Add some npm packages
npm install --global fkill-cli

# Add some pip3 packages
pip3 install install black\[jupyter\] arrow numpy pytz loguru typer openai icecream jupyterlab typer rich seaborn matplotlib pandas ipywidgets altair sklearn plotly pyfiglet jsonpickle
pip3 install pip --upgrade

# what is the obs plugin

brew install --cask karabiner-elements iterm2 alfred bartender alt-tab hammerspoon  visual-studio-code docker vlc
