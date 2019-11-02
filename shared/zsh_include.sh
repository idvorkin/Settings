# shared zsh settings to be sourced
[ -f ~/gits/z/z.sh ]  && source ~/gits/z/z.sh
PATH+=:~/.local/bin
alias gfrall='for git_directory in * ; echo $git_directory && git -C $git_directory fr'
alias dwc='pushd ~/gits/settings && python3 -c "from vim_python import * ;WCDailyPage()" && pushd ~/gits/igor2/750words '
alias dgc='pushd ~/gits/settings && python3 -c "from vim_python import * ;GitCommitDailyPage()" && pushd ~/gits/igor2/750words '
set -o vi
