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


export EDITOR=nvim

# C-T search Files Fuzzy
# C-R Search History fuzzy
source_if_exists ~/.fzf.zsh
source_if_exists ~/homebrew/etc/profile.d/z.sh

PATH+=:~/.local/bin
alias ghg-md-sink='gh gist create --filename=out.md -- '

# Use diff so fancy without needing to be in git
diff-so-fancy() {
  git diff --no-index --color "$@"
}

trim_file_after_marker_to_new_file() {
  local input_file="$1"
  local marker="$2"
  local output_file="$3"

  # Check if the input file exists
  if [[ ! -f "$input_file" ]]; then
    echo "Error: Input file '$input_file' does not exist."
    return 1
  fi

  # Use awk to find the last occurrence of the marker and print from there
  awk -v marker="$marker" '
  {
    if (index($0, marker) == 1) {
      last_marker_line = NR  # Save the line number of the last marker
      last_marker_content = $0  # Save the content of the last marker line
    }
    lines[NR] = $0  # Store each line in an array
  }
  END {
    if (last_marker_line > 0) {
      for (i = last_marker_line; i <= NR; i++) {
        print lines[i]
      }
    }
  }' "$input_file" > "$output_file"
}


function ghg-aider()
{
    trim_file_after_marker_to_new_file '.aider.chat.history.md' '# aider chat started at' '.aider.last.chat.md'
    gh gist create -w .aider.last.chat.md
    echo v0.1
}

alias alf="open '/Applications/Alfred 5.app/'"
function charge() {
    pmset -g batt
    system_profiler SPPowerDataType  | grep Watt
}

gchanges() {
    # for directory in settings, idvorkin.github.io, nlp, tony_tesla, run changs command
    pushd ~/gits
    for git_directory in settings idvorkin.github.io nlp tony_tesla; do
        cd ~/gits
        echo $git_directory
        changes  --directory $git_directory $1 $2 $3 $4
    done

}

alias hibernate="sudo pmset sleepnow"
alias imgls='timg --grid 4 --title'
alias lg='lazygit'
alias gfrall='for git_directory in * ; echo $git_directory && git -C $git_directory fr'
alias gpushall='for git_directory in * ; echo $git_directory && git -C $git_directory push'
alias weather="curl wttr.in/seattle"
alias dwc='pushd ~/gits/settings && python3 -c "from vim_python import * ;WCDailyPage()" && pushd ~/gits/igor2/750words '
alias dgc='pushd ~/gits/settings && python3 -c "from vim_python import * ;GitCommitDailyPage()" && pushd ~/gits/igor2/750words '
alias sl='ssh lightsail'
alias asl='autossh -M 20000 lightsail_no_forward'
alias slnf='ssh lightsail_no_forward'
alias tam='tmux attach-session -t main || tmux new-session -s main'
alias ytsub='youtube-dl --write-sub --sub-format srt --skip-download'
alias xuilocal=" pipxu install -f . --editable"

alias ghe="gh copilot explain"
alias ghs="gh copilot suggest"


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
    life journal-report $1 $2 $3 $4
}


function ijv() {
    ij body --close $1 | vim -
}

call_ijm_for_n() {
  local n=$1
  local i=1
  while [ $i -le $n ]; do
    ijm --no-launch-fx "$i"
    ((i++))
  done
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




# Set alias that are always better
alias_if_exists cat bat
alias_if_exists ranger yazi
alias_if_exists ls exa
alias_if_exists df duf
alias_if_exists top htop
alias_if_exists ndcu gdu
alias_if_exists du dua
alias_if_other_exists cd z zoxide
alias_if_exists ps procs
# On mac, gpt is in sbin, use the installed from idvorkin_nlp version instead
alias_if_exists gpt ~/.local/bin/gpt

# Igor setups use Soed and Sodot as useful aliases
alias Soed='nvim ~/settings/shared/zsh_include.sh'
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

function esecret_jq() {
    export "$1"=$(jq -r .$1 ~/gits/igor2/secretBox.json)
    # list the first 10 chars of the secret, which follows the = sign
    export SCRATCH=`export | grep $1`
    echo ${SCRATCH:0:10},${SCRATCH:20:4}
}

function curl_md() {
    curl $1 | pandoc -f html -t markdown
}

function export_secrets()
{
    esecret_jq LANGCHAIN_API_KEY
    esecret_jq ANTHROPIC_API_KEY
    esecret_jq OPENAI_API_KEY
    esecret_jq VAPI_API_KEY
    esecret_jq GOOGLE_API_KEY
    esecret_jq GROQ_API_KEY
    esecret_jq PPLX_API_KEY
    esecret_jq ZEP_API_KEY
    esecret_jq TONY_STORAGE_SERVER_API_KEY
    esecret_jq TONY_API_KEY
    esecret_jq ASSEMBLYAI_API_KEY
    export BING_SEARCH_URL='https://api.bing.microsoft.com/v7.0/search'
}

echo "Random"

# Some useful work aliases
alias chh='wchat messages'
alias thr='wchat threads'
alias grtd="grep ☐"
# open the output of pbpaste
# alias pbo="open $(pbpaste)"
alias copy_secrets_from_shell='scp lightsail:/home/ec2-user/gits/igor2/secretBox.json ~/gits/igor2/secretBox.json'
function rtd()
{
    ssh lightsail_no_forward cat /home/ec2-user/gits/igor2/750words/$(date +'%Y-%m-%d').md | grep ☐ | pbcopy
    pbpaste
}

function nvday()
{
    nvim scp://ec2-user@lightsail//home/ec2-user/gits/igor2/750words/$(date +'%Y-%m-%d').md
}


# Useful stuff w/OSX Sound
alias restart_audio='sudo launchctl kickstart -kp system/com.apple.audio.coreaudiod'
alias airpods_audio='SwitchAudioSource -s "Igor’s AirPods Pro" && SwitchAudioSource -t input -s "Igor’s AirPods Pro" '

# Turn off auto update brew

export HOMEBREW_NO_AUTO_UPDATE=1

# Setting this allows lip gloss to use truecolor
# Man, what a PITA
export COLORTERM=truecolor

set -o vi
set nobell

#  shared zsh settings to be sourced
# TMUX attach

# I'm not sure why, but ruby can't find the linuxbrew path
# export LD_LIBRARY_PATH=/home/linuxbrew/.linuxbrew/lib:/opt/homebrew/lib
bindkey -M viins 'fj' vi-cmd-mode

source ~/settings/shared/fzf_git_keybindings.zsh
[ -f ~/.fzf.zsh ] && source ~/.fzf.zsh

echo "starting evals"
# TODO: consider doing this in a loop as it's really annoying to have 3 configurations
eval_w_param_if_exists ~/homebrew/bin/brew shellenv
eval_w_param_if_exists /home/linuxbrew/.linuxbrew/bin/brew shellenv
export STARSHIP_CONFIG=~/settings/shared/starship.toml
eval "$(zoxide init zsh)"
# unset MCFLY_DEBUG=
# eval "$(mcfly init zsh)"
eval "$(atuin init zsh --disable-up-arrow)"
eval "$(starship init zsh)"
eval "$(thefuck --alias)"
eval "$(rbenv init -)"

echo ++zfunc
for func in ~/.zfunc/*; do source $func; done
echo --zfunc

export CARAPACE_BRIDGES='zsh'
zstyle ':completion:*' format $'\e[2;37mCompleting %d\e[m'
source <(carapace _carapace)

# Can't activate it in the directory or won't work?
# Some reason need to activate it outside the script -not worth figuring out
alias activate_pyenv=". ~/gits/nlp/.venv/bin/activate"

unalias a # not sure why  a gets an alias
echo "zsh_include complete"

