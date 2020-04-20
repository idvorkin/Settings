export brew_packages="\
ag \
asciinema \
aws-shell \
bat \
cask \
diff-so-fancy \
exa \
fasd \
fd \
ffmpeg \
fselect \
fzf \
git \
git-extras \
git-delta \
glances \
graphviz \
grip \
htop \
hub \
httpie \
imagemagick \
jq \
libevent \
litecli \
mosh \
ncdu \
neofetch \
npm \
openssl \
optipng \
pipenv \
pup \
python3 \
ranger \
rg \
ruby \
s3cmd \
tig \
tmux \
wget \
w3m \
docker \
yasm \
zip \
zsh \
yarn "

# https://unix.stackexchange.com/questions/7558/execute-a-command-once-per-line-of-piped-input
# Ahh magic. xargs takes it's input execute command once per line
echo $brew_packages | xargs -n1 brew install

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

# Add some npm packages
npm install --global fkill-cli
