# This is used by vs codespaces to setup....
# Should be fast

# load up the git repo's for plugin managers
git clone https://github.com/VundleVim/Vundle.vim.git ~/.vim/bundle/Vundle.vim
git clone https://github.com/tmux-plugins/tpm ~/.tmux/plugins/tpm

# shared git stetup
# Hymn, maybe i have this in a git config file instead?
git config --global user.email "idvorkin@gmail.com"
git config --global user.name "Igor Dvorkin"
git config --global push.default simple
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
git config --global push.default simple
git config --global alias.co checkout
git config --global alias.com "checkout master"
git config --global alias.fr "pull --rebase"
git config --global alias.br branch
git config --global alias.ci commit
git config --global alias.st status
git config --global push.autoSetupRemote 1
git config --global alias.logc log master..
git config --global alias.logp "log --pretty=format:'%C(yellow)%h%Cred%d %Creset%s %C(yellow)[%cn] %C(green)(%ar)' --decorate"
ln -s -f ~/settings/shared/gitconfig ~/.gitconfig
ln -s -f ~/settings/shared/tigrc ~/.tigrc
ln -s -f ~/settings/shared/bottomrc.toml ~/.config/bottom/bottom.toml

ln -s -f ~/settings/default_vimrc ~/.vimrc
ln -s -f ~/settings/shared/ranger_rc.conf ~/.config/ranger/rc.conf
mkdir ~/.config
ln -s -f ~/settings/shared/litecli_config ~/.config/litecli/config
ln -s -f ~/settings/mac/.xvimrc ~/.xvimrc
ln -s -f ~/settings/mac/.inputrc ~/.inputrc
ln -s -f ~/settings/tmuxp ~/.tmuxp
mkdir ~/.hammerspoon
mkdir ~/.ssh
mkdir -p ~/.config/bottom

ln -s -f ~/settings/mac/hammerspoon/init.lua ~/.hammerspoon/init.lua
mkdir -p ~/.config/atuin
ln -s -f ~/settings/shared/atuin.toml ~/.config/atuin/config.toml
ln -s -f ~/settings/mac/.finicky.js  ~/.finicky.js
ln -s -f ~/settings/shared/ctags ~/.ctags
ln -s -f ~/settings/shared/.tmux.conf ~/.tmux.conf
ln -s -f ~/settings/shared/.tmux.conf ~/.tmux/.tmux.conf
ln -s -f ~/settings/shared/.vimperatorrc ~/.vimperatorrc
ln -s -f ~/settings/shared/ipython_config.py  ~/.ipython/ipython_config.py
ln -s -f ~/settings/shared/ipython_config.py  ~/.ipython/profile_default/ipython_config.py
ln -s -f ~/settings/shared/.ideavim ~/.ideavimrc
ln -s -f ~/settings/shared/aider.conf.yml ~/.aider.conf.yml
ln -s -f ~/settings/shared/ssh_config ~/.ssh/config

# Todo consider enumerating the directory and linking all of them
ln -s -f ~/settings/nvim ~/.config/nvim
ln -s -f ~/settings/config/carapace/ ~/.config/carapace
ln -s -f ~/settings/config/yazi ~/.config/yazi
ln -s -f ~/settings/config/bat ~/.config/bat
ln -s -f ~/settings/config/mcphub ~/.config/mcphub
ln -s -f ~/settings/config/mpv ~/.config/mpv

# Cursor config (official location is ~/.cursor but also link to ~/.config/cursor for consistency)
ln -s -f ~/settings/config/cursor ~/.config/cursor
ln -s -f ~/settings/config/cursor ~/.cursor

# Mac goodies
ln -s -f ~/settings/config/yabai ~/.config/yabai
mkdir ~/.config/borders
ln -s -f ~/settings/config/borders/bordersrc ~/.config/borders/bordersrc
ln -s -f ~/settings/shared/aerospace.toml ~/.aerospace.toml
#
# Link to lots of dot files
mkdir ~/.config/karabiner/
ln -s -f ~/settings/mac/karabiner.json ~/.config/karabiner/karabiner.json
ln -s -f ~/settings/mac/multi_keyboard_sync.json ~/.config/karabiner/assets/complex_modifications/multi_keyboard_sync.json

(echo '# Added by bootstrap.sh') >> ~/.zshrc
(echo 'plugins=(git macos lol vi-mode web-search wd fasd httpie tig tmux fzf)') >> ~/.zshrc
(echo '. ~/settings/shared/zsh_include.sh') >> ~/.zshrc

