# SETUP: Put these in your zshrc
##################

#  Helpful go to your by pressing gf on: ~/.zshrc

# https://github.com/lukechilds/zsh-better-npm-completion
# plugins=(git macos lol vi-mode web-search wd fasd httpie tig tmux fzf)
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
eval_w_param_if_exists /home/linuxbrew/.linuxbrew/bin/brew shellenv

export EDITOR=vim

# C-T search Files Fuzzy
# C-R Search History fuzzy
source_if_exists ~/.fzf.zsh
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
alias mb="pbpaste | sed  's!idvork.in/!idvorkin.azurewebsites.net/!'| sed 's!#!/!' | pbcopy"

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

function journal_gpt() {
    # $1 date
    # $2 command
     ~/igor_journal.py entries $1  | while read line; do  echo && echo \#\#\#\ $line && ~/igor_journal.py body  $line | ~/gpt.py $2 ; done  | tee ~/tmp/$2_for_$1.md
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

function alias_if_other_exists() {
    # $1 - alias
    # $2 - replacement command
    # $3 - command to test for
    which $3 &> /dev/null
    if [[ $? -eq 0 ]] ; then
        alias $1=$2
    else
        echo "program $2 not found"

    fi
}

function do_wsl() {
    echo "in WSL"
    alias pbcopy='clip.exe'

}

if [[ "$(uname -a)" =~ "microsoft" ]]; then
    do_wsl
fi

function gstatdaterange() {
    # $1 - start
    # $2 - end
    # can be days ago
    # glogdate '30 days ago' '1 day ago'
    # or absolute dates
    # glogdate '12/01/2020'

    # output all git commits since until, pretty print to just have the commit
    git_output=`git log --since "$1" --until "$2" --pretty="%H"`

    # diff between first commit to last commit, and sort the output by size
    #sort params -k=second column; -t=with delimter as |; -n=sort as numeric -r sort as reversed
    git diff --stat `echo $git_output | tail -n 1` `echo $git_output | head -n 1` |  sort -k2 -t'|' -n -r
}

function rhyme()
{
     # Call Rhyme using the rhymebrain API
     # jq - https://stedolan.github.io/jq/manual/#Basicfilters
     # rhymebrain API - https://rhymebrain.com/api.html
     http "https://rhymebrain.com/talk?function=getRhymes&word=$1" | jq '.[] | select( .score == 300) |.word'
}

eval "$(zoxide init zsh)"
eval "$(mcfly init zsh)"


# Set alias that are always better
alias_if_exists cat bat
alias_if_exists ls exa
alias_if_exists df duf
alias_if_exists top htop
alias_if_exists ndcu gdu
alias_if_exists du dua
alias_if_other_exists cd z zoxide
alias_if_exists ps procs

# Igor setups use Soed and Sodot as useful aliases
alias Soed='vim ~/settings/shared/zsh_include.sh'
alias Sodot='.  ~/settings/shared/zsh_include.sh'

# Some useful work aliases
alias chh='wchat messages'
alias thr='wchat threads'


set -o vi
set nobell

#  shared zsh settings to be sourced
# TMUX attach

# I'm not sure why, but ruby can't find the linuxbrew path
export LD_LIBRARY_PATH=$(LD_LIBRARY_PATH):/home/linuxbrew/.linuxbrew/lib
export LD_LIBRARY_PATH=$(LD_LIBRARY_PATH):/opt/homebrew/lib
bindkey -M viins 'fj' vi-cmd-mode

source ~/settings/shared/fzf_git_keybindings.zsh
[ -f ~/.fzf.zsh ] && source ~/.fzf.zsh

export STARSHIP_CONFIG=~/settings/shared/starship.toml
eval "$(starship init zsh)"
eval "$(thefuck --alias)"
echo "zsh_include complete"
eval "$(rbenv init -)"
