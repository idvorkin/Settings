
export brew_packages="\
tmux \
zoxide \
bat \
exa \
jq \
mcfly \
fd \
procs \
fzf \
gh \
viu \
rich \
duf \
htop \
ag \
yt-dlp \
gdu \
zsh \
git \
tig \
cpulimit \
httpie \
dua-cli \
ranger \
fasd \
docker \
png-quant \
docker-compose \
diff-so-fancy \
git-extras \
imagemagick \
git-delta \
python3 \
asciinema \
asciiquarium \
docui \
aws-shell \
ffmpeg \
fselect \
glances \
gotop \
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
nmap \
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
curlie \
pandoc \
cask \
timg \
xh \
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
# Start by fetching the packages in parallel as that's non blocking and can run in paralle, while install runs in series
echo $brew_packages | xargs -n1 --max-procs=8 brew fetch
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
python3 -m pip install install black\[jupyter\] arrow numpy pytz loguru typer openai icecream jupyterlab typer rich seaborn matplotlib pandas ipywidgets altair sklearn plotly pyfiglet jsonpickle pandas-datareader nltk fernet cryptography imutils sklearn scikit-learn jupyterlab-vim jupyterlab-vimrc pendulum poetry torch torchvision opencv-python typeguard tiktoken modal-client uvicorn py-cord asyncify asyncer mypy langchain unstructured chromadb


# On the mac can install tensor flow as follows
python3 -m pip install tensorflow-macos


# what is the obs plugin

brew install --cask karabiner-elements iterm2 alfred bartender alt-tab hammerspoon  visual-studio-code docker vlc

# Install from cargo incase on linux docker on osx which does not support bottles
curl https://sh.rustup.rs -sSf | sh
cargo install zoxide bat duf exa mcfly dua procs htop starship dua

# install azure functions

brew tap azure/functions
brew install azure-functions-core-tools@4
# if upgrading on a machine that has 2.x or 3.x installed:
brew link --overwrite azure-functions-core-tools@4
