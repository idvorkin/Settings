#!/home/linuxbrew/.linuxbrew/bin/zsh
#  copied from zsh_include
#  Run cron command every 4 minutes
# */4 * * * * ~/settings/shared/cron_git_sync.sh >>~/tmp/cronrun 2>&1
date
cd ~/gits/
for git_directory in * ; echo $git_directory && git -C $git_directory fr
for git_directory in * ; echo $git_directory && git -C $git_directory push
