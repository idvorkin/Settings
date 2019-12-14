# shared zsh settings to be sourced
[ -f ~/gits/z/z.sh ]  && source ~/gits/z/z.sh
PATH+=:~/.local/bin
alias gfrall='for git_directory in * ; echo $git_directory && git -C $git_directory fr'
alias gpushall='for git_directory in * ; echo $git_directory && git -C $git_directory push'
alias dwc='pushd ~/gits/settings && python3 -c "from vim_python import * ;WCDailyPage()" && pushd ~/gits/igor2/750words '
alias dgc='pushd ~/gits/settings && python3 -c "from vim_python import * ;GitCommitDailyPage()" && pushd ~/gits/igor2/750words '

unalias ddg
function ddg() {
    pushd ~/gits/linqpadsnippets/python
    python3 dump_grateful.py grateful --days $1
    popd
}
function dda() {
    pushd ~/gits/linqpadsnippets/python
    python3 dump_grateful.py awesome --days $1
    popd
}

function ddt() {
    pushd ~/gits/linqpadsnippets/python
    python3 dump_grateful.py todo --days $1
    popd
}

function dda() {
    pushd ~/gits/linqpadsnippets/python
    python3 dump_grateful.py --awesome $1
    popd
}


set -o vi
