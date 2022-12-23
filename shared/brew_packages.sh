
export brew_packages="\
zoxide \
exa \
jq \
mcfly \
fzf \
bat \
rich \
duf \
ag \
fd \
zsh \
tmux \
git \
tig \
docui \
httpie \
dua-cli \
ranger \
cask \
fasd \
docker \
docker-compose \
diff-so-fancy \
git-extras \
imagemagick \
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


# to execute things from VIM
# <range>w !bash

# https://unix.stackexchange.com/questions/7558/execute-a-command-once-per-line-of-piped-input
# Ahh magic. xargs takes it's input execute command once per line
echo $brew_packages | xargs -n1 brew install

#  Get latest version of mosh
brew install mosh --head

# Correct version of tags
brew install --HEAD universal-ctags/universal-ctags/universal-ctags

# currently broken on some devices.
brew install azure-cli cmatrix iftop

# ranger = File Explorer
# grv - get repository viewer
# grv need to unalias grv in zsh


# Add some npm packages
npm install --global fkill-cli

# Add some pip3 packages
python3 -m pip install install black\[jupyter\] arrow numpy pytz loguru typer openai icecream jupyterlab typer rich seaborn matplotlib pandas ipywidgets altair sklearn plotly pyfiglet jsonpickle pandas-datareader nltk fernet

# what is the obs plugin

brew install --cask karabiner-elements iterm2 alfred bartender alt-tab hammerspoon  visual-studio-code docker vlc

# Install from cargo incase on linux docker on osx which does not support bottles
curl https://sh.rustup.rs -sSf | sh
cargo install zoxide bat duf exa mcfly dua procs htop starship dua
