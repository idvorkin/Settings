#!/bin/zsh

function source_if_exists() {
    [ -f "$1" ] && source "$1"
}

function eval_w_param_if_exists() {
    if [ -f "$1" ] || command -v "$1" >/dev/null 2>&1; then
        eval "$("$1" "${@:2}")"
    else
        # echo "Error: '$1' is neither a valid file nor a recognized command."
        return 0
    fi
}

function charge() {
    pmset -g batt
    system_profiler SPPowerDataType | grep Watt
}

function trim_file_after_marker_to_new_file() {
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
            last_marker_line = NR
            last_marker_content = $0
        }
        lines[NR] = $0
    }
    END {
        if (last_marker_line > 0) {
            for (i = last_marker_line; i <= NR; i++) {
                print lines[i]
            }
        }
    }' "$input_file" > "$output_file"
}

function ghg-aider() {
    trim_file_after_marker_to_new_file '.aider.chat.history.md' '# aider chat started at' '.aider.last.chat.md'
    gh gist create -w .aider.last.chat.md
    echo v0.1
}

function gist-create() {
    # Ensure gh is installed
    if ! command -v gh &> /dev/null; then
        echo "GitHub CLI (gh) is not installed. Please install it first."
        return 1
    fi

    # If files were passed as arguments, use those
    if [ $# -gt 0 ]; then
        # Verify all files exist
        for file in "$@"; do
            if [ ! -f "$file" ]; then
                echo "Error: File '$file' does not exist."
                return 1
            fi
        done

        # Create gist with provided files
        gh gist create -w "$@"
        echo "Gist created successfully!"
        return 0
    fi

    # If no files provided, use interactive selection
    # Ensure fzf is installed for interactive mode
    if ! command -v fzf &> /dev/null; then
        echo "fzf is not installed. Please install it first."
        return 1
    fi

    # Let user select files using fzf
    local selected_files=$(find . -type f -not -path '*/\.*' | fzf --multi --height 40% --reverse)

    if [[ -z "$selected_files" ]]; then
        echo "No files selected"
        return 0
    fi

    # Create gist with selected files
    echo "$selected_files" | xargs gh gist create -w

    echo "Gist created successfully!"
}

function gist-clone() {
    # Ensure gh is installed
    if ! command -v gh &> /dev/null; then
        echo "GitHub CLI (gh) is not installed. Please install it first."
        return 1
    fi

    # Ensure fzf is installed
    if ! command -v fzf &> /dev/null; then
        echo "fzf is not installed. Please install it first."
        return 1
    fi

    # Create base directory if it doesn't exist
    local base_dir="$HOME/tmp/gists"
    mkdir -p "$base_dir"

    # Get gist selection using fzf
    local selected_gist=$(gh gist list | fzf --height 40% --reverse)

    if [[ -z "$selected_gist" ]]; then
        echo "No gist selected"
        return 0
    fi

    # Extract gist ID and description
    local gist_id=$(echo "$selected_gist" | awk '{print $1}')
    local description=$(echo "$selected_gist" | cut -f2-)

    # Create safe directory name from description:
    # - Take first 40 chars
    # - Convert to lowercase
    # - Replace spaces and special chars with hyphens
    # - Remove leading/trailing hyphens
    local dir_name=$(echo "$description" |
        cut -c1-40 |
        tr '[:upper:]' '[:lower:]' |
        sed 's/[^a-z0-9]/-/g' |
        sed 's/--*/-/g' |
        sed 's/^-\|-$//g')

    # Use gist ID if no valid directory name could be created
    if [[ -z "$dir_name" ]]; then
        dir_name="$gist_id"
    fi

    local target_dir="$base_dir/$dir_name"

    # Create target directory and clone gist
    if mkdir -p "$target_dir"; then
        echo "Cloning gist into $target_dir..."
        gh gist clone "$gist_id" "$target_dir"
        echo "Done! Gist cloned to $target_dir"
        cd "$target_dir"
    else
        echo "Failed to create directory $target_dir"
        return 1
    fi
}

function gist-open() {
    # First try to get gist ID from current directory if it's a git repo
    if git rev-parse --git-dir > /dev/null 2>&1; then
        # Check if this is a gist repository by looking at the remote URL
        local gist_id=$(git remote -v | grep fetch | grep 'gist.github.com' | sed -E 's/.*\/([a-f0-9]+)\.git.*/\1/')

        if [[ ! -z "$gist_id" ]]; then
            echo "Opening gist from current directory..."
            gh gist view --web "$gist_id"
            return 0
        fi
    fi

    # If not in a gist directory, show list of gists to choose from
    echo "Not in a gist directory, selecting from list..."

    # Ensure fzf is installed
    if ! command -v fzf &> /dev/null; then
        echo "fzf is not installed. Please install it first."
        return 1
    fi

    # Get gist selection using fzf
    local selected_gist=$(gh gist list | fzf --height 40% --reverse)

    if [[ -z "$selected_gist" ]]; then
        echo "No gist selected"
        return 0
    fi

    # Extract gist ID and open it
    local gist_id=$(echo "$selected_gist" | awk '{print $1}')
    gh gist view --web "$gist_id"
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
function echomb() {
    echo $1 > ~/tmp/mb.in
    cat ~/tmp/mb_tmp | sed  's!idvork.in/!idvorkin.azurewebsites.net/!'| sed 's!#!/!'  > ~/tmp/mb.out
    cat ~/tmp/mb.out | pbc
    cat ~/tmp/mb.out
}
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
    #sort params -k=second column; -t=with delimiter as |; -n=sort as numeric -r sort as reversed
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
    pkill -9 Chrome
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

function rtd()
{
    ssh lightsail_no_forward cat /home/ec2-user/gits/igor2/750words/$(date +'%Y-%m-%d').md | grep ☐ | pbcopy
    pbpaste
}

function nvday()
{
    nvim scp://ec2-user@lightsail//home/ec2-user/gits/igor2/750words/$(date +'%Y-%m-%d').md
}


function dumpPrompts() {
    # Use an array to explicitly handle glob expansion
    local files=($1)
    echo files

    for file in "${files[@]}"; do
        echo $file
        cat "$file" | dasel -r json '.Recommendations.all().property(PromptToUseDuringReflection?)'
    done
}

function imgls()
{
    if [[ -n "$TMUX" ]]; then
        timg --grid 4 --title -ps "$@"
    else
        timg --grid 4 --title "$@"
    fi
}

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
    # Skip echo since too noisy
    # echo ${SCRATCH:0:10},${SCRATCH:20:4}
}

function curl_md() {
    curl $1 | pandoc -f html -t markdown
}

function export_secrets()
{
    esecret_jq LANGCHAIN_API_KEY
    esecret_jq ANTHROPIC_API_KEY
    esecret_jq OPENAI_API_KEY
    esecret_jq IFTTT_WEBHOOK_KEY
    esecret_jq DEEPGRAM_API_KEY
    esecret_jq IFTTT_WEBHOOK_SMS_EVENT
    esecret_jq TWILIO_ACCOUNT_SID
    esecret_jq TWILIO_AUTH_TOKEN
    esecret_jq TWILIO_FROM_NUMBER
    esecret_jq EXA_API_KEY
    esecret_jq GITHUB_PERSONAL_ACCESS_TOKEN
    esecret_jq VAPI_API_KEY
    esecret_jq GOOGLE_API_KEY
    esecret_jq GROQ_API_KEY
    esecret_jq PPLX_API_KEY
    esecret_jq ZEP_API_KEY
    esecret_jq TONY_STORAGE_SERVER_API_KEY
    esecret_jq TONY_API_KEY
    esecret_jq ASSEMBLYAI_API_KEY
    esecret_jq REPLICATE_API_TOKEN
    esecret_jq ELEVEN_API_KEY
    esecret_jq ONEBUSAWAY_API_KEY
    export BING_SEARCH_URL='https://api.bing.microsoft.com/v7.0/search'
}
#
# Use diff so fancy without needing to be in git
diff-so-fancy() {
  git diff --no-index --color "$@"
}

function safe_init()
{
    echo ++safe_init

    export EDITOR=nvim
    PATH+=:~/.local/bin:~/.cargo/bin
    alias tam='tmux attach-session -t main || tmux new-session -s main'
    alias tas='tmux attach-session -t servers || tmux new-session -s servers'
    #
    # Set alias that are always better
    alias_if_exists cat bat
    alias_if_exists dua gdu-go
    alias_if_exists ranger yazi
    alias_if_exists ls eza
    alias_if_exists df duf
    alias_if_exists top btm
    alias_if_exists ndcu gdu
    alias_if_exists du dua
    alias_if_exists neofetch fastfetch
    alias_if_other_exists cd z zoxide
    alias_if_exists ps procs
    # On mac, gpt is in sbin, use the installed from idvorkin_nlp version instead
    alias_if_exists gpt ~/.local/bin/gpt

    # Igor setups use Soed and Sodot as useful aliases
    alias Soed='nvim ~/settings/shared/zsh_include.sh'
    alias Sodot='.  ~/settings/shared/zsh_include.sh'
    alias claude='unalias -a; command claude'
    alias claude-code='unalias -a; npx @anthropic-ai/claude-code@latest'
    
    # Launch Claude in a new shell with GitHub environment
    function claude-gh() {
        # Get GitHub token from 1Password or manual input
        local gh_token=""
        
        # Try 1Password first
        if command -v op &>/dev/null; then
            gh_token=$(op read "op://Personal/GitHub AI Personal Access Token/token" 2>/dev/null)
        fi
        
        # If no token from 1Password, ask for manual input
        if [[ -z "$gh_token" ]]; then
            echo "Enter GitHub token (or press Enter to continue without):"
            read -s gh_token
        fi
        
        # Start a new shell with clean environment and GitHub token
        env -i \
            HOME="$HOME" \
            PATH="$PATH" \
            TERM="xterm-256color" \
            COLORTERM="truecolor" \
            SHELL="$SHELL" \
            USER="$USER" \
            LANG="$LANG" \
            LC_ALL="$LC_ALL" \
            EDITOR="${EDITOR:-vim}" \
            TMPDIR="$TMPDIR" \
            SSH_AUTH_SOCK="$SSH_AUTH_SOCK" \
            GITHUB_TOKEN="$gh_token" \
            GH_TOKEN="$gh_token" \
            GIT_AUTHOR_NAME="[AI] Igor Dvorkin" \
            GIT_AUTHOR_EMAIL="idvorkin.ai.tools@gmail.com" \
            GIT_COMMITTER_NAME="[AI] Igor Dvorkin" \
            GIT_COMMITTER_EMAIL="idvorkin.ai.tools@gmail.com" \
            bash -c 'exec claude'
    }
    export COLORTERM=truecolor

    set -o vi
    set nobell

    echo "++zfunc"
    if [ -d ~/.zfunc ] && [ "$(ls -A ~/.zfunc)" ]; then
      for func in ~/.zfunc/*; do
        source $func
      done
    else
      echo "No functions found in ~/.zfunc"
    fi
    echo "--zfunc"

    # Can't activate it in the directory or won't work?
    # Some reason need to activate it outside the script -not worth figuring out
    alias activate_env=". .venv/bin/activate"
    alias nbdiffcode="nbdiff --ignore-metadata --ignore-details --ignore-output"

    echo "++eval"
    # TODO: consider doing this in a loop as it's really annoying to have 3 configurations
    eval_w_param_if_exists ~/homebrew/bin/brew shellenv
    eval_w_param_if_exists /opt/homebrew/bin/brew shellenv

    eval_w_param_if_exists /home/linuxbrew/.linuxbrew/bin/brew shellenv
    eval_w_param_if_exists /opt/homebrew/.linuxbrew/bin/brew shellenv
    eval_w_param_if_exists /brew shellenv
    export STARSHIP_CONFIG=~/settings/shared/starship.toml
    eval_w_param_if_exists zoxide init zsh
    eval_w_param_if_exists starship init zsh
    # unset MCFLY_DEBUG=
    # eval "$(mcfly init zsh)"
    eval_w_param_if_exists atuin init zsh --disable-up-arrow
    eval_w_param_if_exists thefuck --alias
    eval_w_param_if_exists rbenv init -
    eval_w_param_if_exists dasel completion zsh
    echo "--eval"

    echo --safe_init

} # end safe init


function default_init() {

echo  ++default_init


# C-T search Files Fuzzy
# C-R Search History fuzzy
source_if_exists ~/.fzf.zsh

alias ghg-md-sink='gh gist create --filename=out.md -- '


alias alf="open '/Applications/Alfred 5.app/'"
alias hibernate="sudo pmset sleepnow"
alias lg='lazygit'
alias recapture='fc -e - |& pbc'
alias gfrall='for git_directory in * ; echo $git_directory && git -C $git_directory fr'
alias gpushall='for git_directory in * ; echo $git_directory && git -C $git_directory push'

function pgfrall() {
    local errors=()
    for git_directory in *; do
        if [ -d "$git_directory/.git" ]; then
            (git -C "$git_directory" fr 2>&1 || echo "ERROR in $git_directory: $?") &
        fi
    done
    wait
    for git_directory in *; do
        if [ -d "$git_directory/.git" ]; then
            local error_output=$(git -C "$git_directory" fr 2>&1 >/dev/null)
            if [ $? -ne 0 ]; then
                errors+=("Error in $git_directory: $error_output")
            fi
        fi
    done
    if [ ${#errors[@]} -ne 0 ]; then
        echo "\nErrors occurred:"
        printf '%s\n' "${errors[@]}"
    fi
}

function pgpushall() {
    local errors=()
    for git_directory in *; do
        if [ -d "$git_directory/.git" ]; then
            (git -C "$git_directory" push 2>&1 || echo "ERROR in $git_directory: $?") &
        fi
    done
    wait
    for git_directory in *; do
        if [ -d "$git_directory/.git" ]; then
            local error_output=$(git -C "$git_directory" push 2>&1 >/dev/null)
            if [ $? -ne 0 ]; then
                errors+=("Error in $git_directory: $error_output")
            fi
        fi
    done
    if [ ${#errors[@]} -ne 0 ]; then
        echo "\nErrors occurred:"
        printf '%s\n' "${errors[@]}"
    fi
}

alias weather="curl wttr.in/seattle"
alias dwc='pushd ~/gits/settings && python3 -c "from vim_python import * ;WCDailyPage()" && pushd ~/gits/igor2/750words '
alias dgc='pushd ~/gits/settings && python3 -c "from vim_python import * ;GitCommitDailyPage()" && pushd ~/gits/igor2/750words '
alias sl='ssh lightsail'
alias asl='autossh -M 20000 lightsail_no_forward'
alias slnf='ssh lightsail_no_forward'
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

if [[ "$(uname -a)" =~ "microsoft" ]]; then
    do_wsl
fi


echo "Random"

# Some useful work aliases
alias chh='wchat messages'
alias thr='wchat threads'
alias grtd="grep ☐"
# open the output of pbpaste
# alias pbo="open $(pbpaste)"
alias copy_secrets_from_shell='scp lightsail:/home/ec2-user/gits/igor2/secretBox.json ~/gits/igor2/secretBox.json'

# Useful stuff w/OSX Sound
alias restart_audio='sudo launchctl kickstart -kp system/com.apple.audio.coreaudiod'
alias airpods_audio='SwitchAudioSource -s "Igor's AirPods Pro" && SwitchAudioSource -t input -s "Igor's AirPods Pro" '

# Better Display cli is the only way fix my resolution
# https://github.com/waydabber/betterdisplaycli?tab=readme-ov-file
alias lg-fix='betterdisplaycli set  --resolution=3840x2160 --refreshRate=59.94Hz '
alias lg-show='betterdisplaycli get  --resolution --refreshRate'

# Function to select from display resolution presets
function lg-presets() {
    local presets=(
        "4K (3840x2160 @ 59.94Hz)"
        "1440p (2560x1440 @ 60Hz)"
        "1080p (1920x1080 @ 60Hz)"
        "Custom"
        "Show current"
    )

    # Use fzf to select a preset if available, otherwise fall back to select
    if command -v fzf &> /dev/null; then
        local selected=$(printf "%s\n" "${presets[@]}" | fzf --height 40% --reverse --prompt="Select display preset: ")
    else
        echo "Select a display preset:"
        select selected in "${presets[@]}"; do
            break
        done
    fi

    case "$selected" in
        "4K (3840x2160 @ 59.94Hz)")
            betterdisplaycli set --resolution=3840x2160 --refreshRate=59.94Hz
            echo "Set display to 4K (3840x2160 @ 59.94Hz)"
            ;;
        "1440p (2560x1440 @ 60Hz)")
            betterdisplaycli set --resolution=2560x1440 --refreshRate=60Hz
            echo "Set display to 1440p (2560x1440 @ 60Hz)"
            ;;
        "1080p (1920x1080 @ 60Hz)")
            betterdisplaycli set --resolution=1920x1080 --refreshRate=60Hz
            echo "Set display to 1080p (1920x1080 @ 60Hz)"
            ;;
        "Custom")
            echo "Enter custom resolution (e.g., 3840x2160):"
            read resolution
            echo "Enter refresh rate (e.g., 60Hz):"
            read refreshRate
            betterdisplaycli set --resolution=$resolution --refreshRate=$refreshRate
            echo "Set display to $resolution @ $refreshRate"
            ;;
        "Show current")
            betterdisplaycli get --resolution --refreshRate
            ;;
        *)
            echo "No preset selected or invalid selection"
            ;;
    esac
}

# Turn off auto update brew

export HOMEBREW_NO_AUTO_UPDATE=1

# Setting this allows lip gloss to use truecolor
# Man, what a PITA

#  shared zsh settings to be sourced
# TMUX attach

# I'm not sure why, but ruby can't find the linuxbrew path
# export LD_LIBRARY_PATH=/home/linuxbrew/.linuxbrew/lib:/opt/homebrew/lib
bindkey -M viins 'fj' vi-cmd-mode

source ~/settings/shared/fzf_git_keybindings.zsh
[ -f ~/.fzf.zsh ] && source ~/.fzf.zsh


export CARAPACE_BRIDGES='zsh'
zstyle ':completion:*' format $'\e[2;37mCompleting %d\e[m'
source <(carapace _carapace)


unalias a # not sure why  a gets an alias
echo  --default_init
}


function blog_think() {
    # Ensure the API key is set
    if [[ -z "$TONY_API_KEY" ]]; then
        echo "Error: TONY_API_KEY is not set."
        return 1
    fi

    # Define the temporary file for the raw response
    local response_file="$HOME/tmp/random_blog_response.json"

    http POST https://idvorkin--modal-blog-server-blog-handler.modal.run \
        x-vapi-secret:$TONY_API_KEY \
        action="random_blog_url_only" > $response_file

    # Check if the request was successful
    if [[ $? -ne 0 ]]; then
        echo "Error: Failed to fetch blog."
        return 1
    fi

    # Extract the `result` field from the response
    local url=`sed -n "s/.*'url': '\([^']*\)'.*/\1/p" $response_file`


    # Check if content and URL were extracted
    if [[ -z "$url" ]]; then
        echo "Error: Failed to extract blog content or URL."
        return 1
    fi

    # Use the trailing path from the URL as the filename
    echo "$url"
    think $url
}


function FinickyGet(){
    defaults read com.apple.LaunchServices/com.apple.launchservices.secure LSHandlers | grep -C 7 "http"

}
function FinickySet(){
    defaults write com.apple.LaunchServices/com.apple.launchservices.secure LSHandlers -array-add '{"LSHandlerURLScheme" = "http"; "LSHandlerRoleAll" = "net.kassett.finicky";}'
    defaults write com.apple.LaunchServices/com.apple.launchservices.secure LSHandlers -array-add '{"LSHandlerURLScheme" = "https"; "LSHandlerRoleAll" = "net.kassett.finicky";}'
}

# Function to update pre-commit hooks in all subdirectories
precommit-update-all() {
  find . -type f -name ".pre-commit-config.yaml" | while read -r config_file; do
    local dir
    dir=$(dirname "$config_file")
    echo "Processing $dir..."
    precommit-update-dir "$dir"
  done

  echo "Done updating all pre-commit configurations."
}

# Function to update pre-commit hooks within a specific directory
precommit-update-dir() {
  local dir="$1"
  cd "$dir" || return 1

  # Stash existing changes if needed
  if ! git diff --quiet; then
    echo "Stashing existing changes..."
    git stash push -m "pre-commit-update"
  fi

  # Run pre-commit autoupdate
  pre-commit autoupdate

  # Check if updates were made and commit changes
  if ! git diff --quiet; then
    git add .pre-commit-config.yaml
    git commit -m "pre-commit autoupdate to latest"
    echo "Committed updates for $dir."
  else
    echo "No updates were made for $dir."
  fi

  # Restore stashed changes if there were any
  if git stash list | grep -q "pre-commit-update"; then
    echo "Restoring stashed changes..."
    git stash pop
  fi

  cd - > /dev/null
}


safe_init
default_init
export_secrets

