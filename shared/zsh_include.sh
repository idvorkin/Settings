# SETUP: Put these in your zshrc
##################

#  Helpful go to your by pressing gf on: ~/.zshrc

# https://github.com/lukechilds/zsh-better-npm-completion
# plugins=(git macos lol vi-mode web-search wd fasd httpie tig tmux fzf gh)
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
alias ghgmd='gh gist create --filename=out.md --'
alias imgls='timg --grid 4 --title'
alias gfrall='for git_directory in * ; echo $git_directory && git -C $git_directory fr'
alias gpushall='for git_directory in * ; echo $git_directory && git -C $git_directory push'
alias weather="curl wttr.in/seattle"
alias dwc='pushd ~/gits/settings && python3 -c "from vim_python import * ;WCDailyPage()" && pushd ~/gits/igor2/750words '
alias dgc='pushd ~/gits/settings && python3 -c "from vim_python import * ;GitCommitDailyPage()" && pushd ~/gits/igor2/750words '
alias sl='ssh lightsail'
alias ytsub='youtube-dl --write-sub --sub-format srt --skip-download'

# Looks like pbcopy just works over ssh/mosh now!
# alias rpbcopy='~/settings/rpbcopy.sh'
alias rpbpaste='~/settings/rpbpaste.sh'
# alias rpbc='~/settings/rpbcopy.sh'
# alias rpbc='~/settings/pbcopy'
alias rpbp='~/settings/rpbpaste.sh'
alias pbc='pbcopy'
alias pbp='pbpaste'
# Use rich markdown pager
alias rmp='rich - -m'

alias tmuxp="~/.local/bin/tmuxp"
alias mb="pbpaste | sed  's!idvork.in/!idvorkin.azurewebsites.net/!'| sed 's!#!/!' | pbcopy"
function echomb() {
    echo $1 > ~/tmp/mb.in
    cat ~/tmp/mb_tmp | sed  's!idvork.in/!idvorkin.azurewebsites.net/!'| sed 's!#!/!'  > ~/tmp/mb.out
    cat ~/tmp/mb.out | pbc
    cat ~/tmp/mb.out
}

unalias ddg
function ddg() {
    dg grateful $1
}

function ddt() {
    dg todo $1
}

function dda() {
    dg awesome $1
}

function ijm() {
    export file_name=~/tmp/ijm/ijm_$1_`date +%y-%m-%d_%H:%M:%S.md`
    ij body --close $1 | ~/gits/nlp/life.py journal-report --debug --u4 | tee $file_name && cat $file_name | rmp
    echo $file_name
}


function ijv() {
    ij body --close $1 | vim -
}

function journal_gpt() {
    # $1 date
    # $2 command
     ~/igor_journal.py entries $1  | while read line; do  echo && echo \#\#\#\ $line && ~/igor_journal.py body  $line | ~/gits/nlp/mood.py $2 ; done  | tee ~/tmp/$2_for_$1.md
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
function go_home()
{
    pkill -9 Workplace
    pkill Outlook
    # figure out how to kill the tabs ...
}

function go_work()
{
    open -g '/Applications/Microsoft Outlook.app'
    open -g '/Applications/Workplace Chat.app'
    # figure out how to kill the tabs ...
    work_cli
}
function work_cli()
{
    wchat threads
    cal dump
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

function png_shrink()
{
    pngquant -f -o $1 $1
}

function cs_install_brew()
{
    # install brew - much faster to default location
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

    (echo; echo 'eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"') >> /home/codespace/.profile
    eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
}

function pbfix()
{
    pbpaste > ~/tmp/gpt.ipc.in
    cat ~/tmp/gpt.ipc.in | ~/gpt3.py fix | tee ~/tmp/gpt.ipc.out
    delta  ~/tmp/gpt.ipc.in ~/tmp/gpt.ipc.out
    echo "Press any key to continue..."
    read
    cat ~/tmp/gpt.ipc.out | pbcopy
    echo "Clipboard updated, you can back original by running"
    echo "~/tmp/gpt.ipc.in | pbcopy"
}

function export_secrets()
{
    export OPENAI_API_KEY=$(jq -r '.openai' ~/gits/igor2/secretBox.json)
    export LANGCHAIN_API_KEY=$(jq -r '.langchain_api_key' ~/gits/igor2/secretBox.json)
    export GOOGLE_API_KEY=$(jq -r '.googleapikey' ~/gits/igor2/secretBox.json)
    export BRAVE_SEARCH_API_KEY=$(jq -r '.brave' ~/gits/igor2/secretBox.json)
    export BING_SUBSCRIPTION_KEY=$(jq -r '.bing' ~/gits/igor2/secretBox.json)
    export BING_SEARCH_URL='https://api.bing.microsoft.com/v7.0/search'
    echo $OPENAI_API_KEY
}

# Some useful work aliases
alias chh='wchat messages'
alias thr='wchat threads'




# Turn off auto update brew

export HOMEBREW_NO_AUTO_UPDATE=1


set -o vi
set nobell

#  shared zsh settings to be sourced
# TMUX attach

# I'm not sure why, but ruby can't find the linuxbrew path
# export LD_LIBRARY_PATH=/home/linuxbrew/.linuxbrew/lib:/opt/homebrew/lib
bindkey -M viins 'fj' vi-cmd-mode

source ~/settings/shared/fzf_git_keybindings.zsh
[ -f ~/.fzf.zsh ] && source ~/.fzf.zsh

export STARSHIP_CONFIG=~/settings/shared/starship.toml
eval "$(starship init zsh)"
eval $(thefuck --alias)
eval "$(rbenv init -)"
echo "zsh_include complete"
