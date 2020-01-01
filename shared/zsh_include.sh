##
#
export PATH=/home/linuxbrew/.linuxbrew/bin/:$PATH
[ -f ~/gits/z/z.sh ]  && source ~/gits/z/z.sh
PATH+=:~/.local/bin
alias gfrall='for git_directory in * ; echo $git_directory && git -C $git_directory fr'
alias gpushall='for git_directory in * ; echo $git_directory && git -C $git_directory push'
alias dwc='pushd ~/gits/settings && python3 -c "from vim_python import * ;WCDailyPage()" && pushd ~/gits/igor2/750words '
alias dgc='pushd ~/gits/settings && python3 -c "from vim_python import * ;GitCommitDailyPage()" && pushd ~/gits/igor2/750words '

alias tmuxp="~/.local/bin/tmuxp"

unalias ddg
function ddg() {
    pushd ~/gits/linqpadsnippets/python
    python3 dump_grateful.py grateful $1
    popd
}

function ddt() {
    pushd ~/gits/linqpadsnippets/python
    python3 dump_grateful.py todo $1
    popd
}

function dda() {
    pushd ~/gits/linqpadsnippets/python
    python3 dump_grateful.py awesome $1
    popd
}


set -o vi
# shared zsh settings to be sourced
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
