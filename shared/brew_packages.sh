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
python3 \
ranger \
rg \
ruby \
s3cmd \
tig \
tmux \
wget \
yasm \
zsh \
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

# Add some npm packages
npm install --global fkill-cli
