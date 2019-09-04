# shared zsh settings to be sourced
. ~/gits/z/z.sh
PATH+=:~/.local/bin
alias gfrall='for git_directory in * ; echo $git_directory && git -C $git_directory fr'
set -o vi
