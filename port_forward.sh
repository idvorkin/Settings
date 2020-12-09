# Some problem with SSH interactive mode, use mosh and seperate ssh instance for port forwarding
# This script uses lightsail which has port forwarding, and a blocking command with no output
ssh lightsail /home/ec2-user/homebrew/bin/nc -l -p 1201

