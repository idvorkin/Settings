# SETUP: Put these in your zshrc
##################

#  Helpful go to your by pressing gf on: ~/.zshrc

# plugins=(git osx lol quote vi-mode web-search wd fasd httpie tig tmux fzf)
# . ~/settings/shared/zsh_include.sh

# Source Brew
# Brew default

function source_if_exists() {
    [ -f $1 ] && source $1
}
function eval_w_param_if_exists() {
    [ -f $1 ] && eval $($1 $2)
}

# TODO: consider doing this in a loop as it's really annoying to have 3 configurations
eval_w_param_if_exists ~/homebrew/bin/brew shellenv
eval_w_param_if_exists /home/linux/.linuxbrew/bin/brew shellenv

export EDITOR=vim

# C-T search Files Fuzzy
# C-R Search History fuzzy
source_if_exists ~/.fzf.zsh
source_if_exists ~/gits/z/z.sh
source_if_exists ~/homebrew/etc/profile.d/z.sh

PATH+=:~/.local/bin
alias gfrall='for git_directory in * ; echo $git_directory && git -C $git_directory fr'
alias gpushall='for git_directory in * ; echo $git_directory && git -C $git_directory push'
alias weather="curl wttr.in/seattle"
alias dwc='pushd ~/gits/settings && python3 -c "from vim_python import * ;WCDailyPage()" && pushd ~/gits/igor2/750words '
alias dgc='pushd ~/gits/settings && python3 -c "from vim_python import * ;GitCommitDailyPage()" && pushd ~/gits/igor2/750words '
alias sl='ssh lightsail'
alias rpbcopy='nc localhost 5726'

alias tmuxp="~/.local/bin/tmuxp"

unalias ddg
function ddg() {
    python3 ~/gits/linqpadsnippets/python/dump_grateful.py grateful $1
}

function ddt() {
    python3 ~/gits/linqpadsnippets/python/dump_grateful.py todo $1
}

function dda() {
    python3 ~/gits/linqpadsnippets/python/dump_grateful.py awesome $1
}

function alias_if_exists() {
    # $1 - alias
    # $2 - replacement command
    which $2 &> /dev/null
    if [[ $? -eq 0 ]] ; then
        alias $1=$2
    else
        echo "program $2 not found"

    fi
}

# Set alias that are always better
alias_if_exists cat bat
alias_if_exists ls exa
alias_if_exists top htop
alias_if_exists tig lazygit

# Igor setups use Soed and Sodot as useful aliases
alias Soed='vim ~/settings/shared/zsh_include.sh'
alias Sodot='.  ~/settings/shared/zsh_include.sh'



set -o vi
echo "zsh_include complete"
#  shared zsh settings to be sourced
# TMUX attach
#
# if [ -z "$TMUX" ]; then
        # tmux attach -t main || tmuxp load simple
# fi
#
# Storing other just in case stuff
# eval "$(fasd --init auto)"
# export PATH=$PATH:$HOME/dotnet
# export LANGUAGE=en_US.UTF-8
# export LANG=en_US.UTF-8
# export LC_ALL=en_US.UTF-8
# export PATH="$PATH:$HOME/.dotnet/tools"

