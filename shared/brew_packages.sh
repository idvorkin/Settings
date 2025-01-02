
export brew_packages="\
tmux \
zoxide \
eza \
bat \
jq \
fd \
procs \
lazygit \
fzf \
duf \
htop \
ag \
yt-dlp \
gdu \
zsh \
git \
yq \
cpulimit \
dua-cli \
docker \
google-cloud-sdk \
png-quant \
docker-compose \
lua-language-server \
diff-so-fancy \
fx \
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
htop \
btop \
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
cask \
xh \
yasm \
zip \
yarn \
rust \
rbenv \
yarn \
difftastic \
pngpaste \
nbdime "


# http downloaders
brew_packages="$brew_packages curlie" # Curl with a UI
brew_packages="$brew_packages httpie" # Another downloader, cli is http

# Data editors
brew_packages="$brew_packages dasel" # jq replacement

brew_packages="$brew_packages neofetch" # Show system info

# Terminal based iamages viewers
brew_packages="$brew_packages viu"
brew_packages="$brew_packages timg"

# Source code tools
brew_packages="$brew_packages gh" # official github cli
brew_packages="$brew_packages hub" # Legacy github cli
brew_packages="$brew_packages cloc" # count lines of code
brew_packages="$brew_packages tig" # git history viewer

brew_packages="$brew_packages pandoc" # Convert between file formats

# Markdown viewer, should include bat
brew_packages="$brew_packages glow"
brew_packages="$brew_packages rich"

brew_packages="$brew_packages yazi" # CLI file viewer

brew_packages="$brew_packages autossh"  # auto reconnect - like MOSH, but works better w/NVIM

brew_packages="$brew_packages atuin"  # shell history, like mcfly, but slower
brew_packages="$brew_packages pipx uv "  # uv is a replacement for pip, and WAAAAAY faster, especially usefulf or pix replacement

brew_packages="$brew_packages mpv" # Great video player, mostly works from CLI

echo $brew_packages

pipx install pipxu # pipx upgrade

pipxu install aider-chat # Code Writing helper

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

gh extension install github/gh-copilot


# Add some npm packages
npm install --global fkill-cli

# Add some pip3 packages
python3 -m pip install black\[jupyter\] arrow numpy pytz loguru typer openai icecream jupyterlab jupytext typer rich seaborn matplotlib pandas ipywidgets altair plotly pyfiglet jsonpickle pandas-datareader nltk fernet cryptography imutils scikit-learn jupyterlab-vim jupyterlab-vimrc pendulum poetry  typeguard tiktoken modal-client uvicorn py-cord asyncify asyncer mypy langchain unstructured chromadb wandb pudb tenacity sparklines ruff pre-commit

python3 -m pip install torch torchvision opencv-python
python3 -m pip install jupyterlab-lsp python-lsp-server jupyter-lsp jupyterlab-vim
python3 -m pip install elevenlabs


# On the mac can install tensor flow as follows
python3 -m pip install tensorflow-macos

python3 -m  pip install httpx
# stuff used by the NLP project
python3 -m  pip install langchain langchain-openai langchain_anthropic  openai langchainhub tiktoken wordcloud requests html2text


# Install from cargo incase on linux docker on osx which does not support bottles
curl https://sh.rustup.rs -sSf | sh
cargo install zoxide bat duf eza mcfly dua procs htop starship dua yazi

brew install --cask google-cloud-sdk
brew install dustinblackman/tap/oatmeal

# Install carapace

brew tap rsteube/homebrew-tap
brew install rsteube/tap/carapace


# install azure functions

brew tap azure/functions
brew install azure-functions-core-tools@4
# if upgrading on a machine that has 2.x or 3.x installed:
brew link --overwrite azure-functions-core-tools@4
