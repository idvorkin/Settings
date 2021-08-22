export brew_packages="\
ag \
asciinema \
asciiquarium \
aws-shell \
bat \
cask \
diff-so-fancy \
fasd \
ffmpeg \
fselect \
fzf \
git \
git-extras \
git-delta \
glances \
gotop \
graphviz \
grip \
gdu \
htop \
hub \
httpie \
imagemagick \
jq \
libevent \
litecli \
mcfly \
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
zoxide \
gdu \
curlie \
xh \
procs \
yasm \
zip \
zsh \
yarn \
rust \
exa \
fd \
yarn "

# Notes, rust is super slow to compile, so putting that packages last

# https://unix.stackexchange.com/questions/7558/execute-a-command-once-per-line-of-piped-input
# Ahh magic. xargs takes it's input execute command once per line
echo $brew_packages | xargs -n1 brew install

# to execute things from VIM
# <range>w !bash

# packages I want that don't exist
# brew install saws svg-term

# currently broken on some devices.
brew install azure-cli cmatrix iftop

# ranger = File Explorer
# grv - get repository viewer
# grv need to unalias grv in zsh


# Correct version of tags
brew install --HEAD universal-ctags/universal-ctags/universal-ctags

# Add some npm packages
npm install --global fkill-cli
