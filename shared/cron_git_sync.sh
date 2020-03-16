#!/home/linuxbrew/.linuxbrew/bin/zsh
#  setup
#    update cron with crontab -e
#     Run cron command every 4 minutes
#    */4 * * * * ~/settings/shared/cron_git_sync.sh >>~/tmp/cronrun 2>&1
#    More luck with the following explicit line
#    */4 * * * * /bin/zsh /home/idvorkin/settings/shared/cron_git_sync.sh >>~/tmp/cronrun 2>&1
#  copied from zsh_include
echo running
date
cd ~/gits/
echo fetching ...
for git_directory in * ; echo $git_directory && git -C $git_directory fr --no-verify
echo pushing ..
for git_directory in * ; echo $git_directory && git -C $git_directory push --no-verify
