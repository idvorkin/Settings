export brew_packages="\
cask \
zsh \
libevent \
openssl \
tmux \
ag \
git \
wget \
ncdu \
graphviz \
htop \
python3 \
ranger \
diff-so-fancy \
fzf \
bat \
fd \
ruby \
aws-shell \
jq \
ag \
npm \
mosh \
exa \
asciinema \
rg \
httpie \
pipenv \
git-extras \
fasd \
glances \
neofetch \
s3cmd \
fselect \
yasm \
ffmpeg \
imagemagick \
optipng \
yarn "

echo $brew_packages
brew install $brew_packages

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
