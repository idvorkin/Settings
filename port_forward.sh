#!zsh
# Some problem with SSH interactive mode, use mosh and seperate ssh instance for port forwarding
# This script uses lightsail which has port forwarding, and a blocking command with no output

# I just need a command that blocks, but it becomes a zombie, so need to be careful about leaving it around
# Block for 20 hours
echo Running ssh sleep
ssh lightsail /usr/bin/sleep 36000
echo Done ssh sleeping

