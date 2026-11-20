# This is used by vs codespaces to setup....
# Should be fast

# load up the git repo's for plugin managers
git clone https://github.com/VundleVim/Vundle.vim.git ~/.vim/bundle/Vundle.vim
git clone https://github.com/tmux-plugins/tpm ~/.tmux/plugins/tpm

# ~/.gitconfig is a link to shared/gitconfig; per-machine settings go in ~/.gitconfig.local
ln -sfn ~/settings/shared/gitconfig ~/.gitconfig
ln -sfn ~/settings/shared/tigrc ~/.tigrc
ln -sfn ~/settings/shared/bottomrc.toml ~/.config/bottom/bottom.toml

ln -sfn ~/settings/default_vimrc ~/.vimrc
ln -sfn ~/settings/shared/ranger_rc.conf ~/.config/ranger/rc.conf
mkdir ~/.config
ln -sfn ~/settings/shared/litecli_config ~/.config/litecli/config
ln -sfn ~/settings/mac/.xvimrc ~/.xvimrc
ln -sfn ~/settings/mac/.inputrc ~/.inputrc
ln -sfn ~/settings/tmuxp ~/.tmuxp
mkdir ~/.hammerspoon
mkdir ~/.ssh
mkdir -p ~/.config/bottom

ln -sfn ~/settings/mac/hammerspoon/init.lua ~/.hammerspoon/init.lua
mkdir -p ~/.config/atuin
ln -sfn ~/settings/shared/atuin.toml ~/.config/atuin/config.toml
ln -sfn ~/settings/mac/.finicky.js  ~/.finicky.js
ln -sfn ~/settings/shared/ctags ~/.ctags
ln -sfn ~/settings/shared/.tmux.conf ~/.tmux.conf
ln -sfn ~/settings/shared/.tmux.conf ~/.tmux/.tmux.conf
ln -sfn ~/settings/shared/.vimperatorrc ~/.vimperatorrc
ln -sfn ~/settings/shared/ipython_config.py  ~/.ipython/ipython_config.py
ln -sfn ~/settings/shared/ipython_config.py  ~/.ipython/profile_default/ipython_config.py
ln -sfn ~/settings/shared/.ideavim ~/.ideavimrc
ln -sfn ~/settings/shared/aider.conf.yml ~/.aider.conf.yml
ln -sfn ~/settings/shared/ssh_config ~/.ssh/config

# Todo consider enumerating the directory and linking all of them
ln -sfn ~/settings/nvim ~/.config/nvim
ln -sfn ~/settings/config/carapace/ ~/.config/carapace
ln -sfn ~/settings/config/yazi ~/.config/yazi
ln -sfn ~/settings/config/bat ~/.config/bat
ln -sfn ~/settings/config/mcphub ~/.config/mcphub
ln -sfn ~/settings/config/mpv ~/.config/mpv
ln -sfn ~/settings/config/ghostty ~/.config/ghostty

# herdr keeps logs/sockets in ~/.config/herdr, so link only the config file
mkdir -p ~/.config/herdr
ln -sfn ~/settings/config/herdr/config.toml ~/.config/herdr/config.toml

# Mac goodies
ln -sfn ~/settings/config/yabai ~/.config/yabai
mkdir ~/.config/borders
ln -sfn ~/settings/config/borders/bordersrc ~/.config/borders/bordersrc
ln -sfn ~/settings/shared/aerospace.toml ~/.aerospace.toml
#
# Link to lots of dot files
mkdir ~/.config/karabiner/
ln -sfn ~/settings/mac/karabiner.json ~/.config/karabiner/karabiner.json
ln -sfn ~/settings/mac/multi_keyboard_sync.json ~/.config/karabiner/assets/complex_modifications/multi_keyboard_sync.json

# No plugins= line here: appended after `source $ZSH/oh-my-zsh.sh` it is dead
# code. Set plugins in ~/.zshrc before oh-my-zsh is sourced.
if ! grep -qF '. ~/settings/shared/zsh_include.sh' ~/.zshrc 2>/dev/null; then
    echo '# Added by bootstrap.sh' >> ~/.zshrc
    echo '. ~/settings/shared/zsh_include.sh' >> ~/.zshrc
fi

# Install UV first (required for Python package management)
echo "Installing UV..."
brew install uv

# Install Python packages via our Python script
echo "Installing Python tools..."
~/settings/py/install_packages.py install

# GitHub HTTPS auth via gh for plain git, SwiftPM and xcodebuild; written to the per-machine file
gh auth status >/dev/null 2>&1 && GIT_CONFIG_GLOBAL=~/.gitconfig.local gh auth setup-git

# Clone active repos into ~/gits and run their installers (needs gh auth login for private repos)
echo "Installing repos..."
~/settings/py/install_repos.py install || echo "Some repos failed; rerun ~/settings/py/install_repos.py install"

