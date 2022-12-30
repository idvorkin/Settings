#!zsh
# Use clipbord over ssh.
# Need to use ncat (installed via brew install nmap), as more recent then netcat.
# Remote Port forwarding already configured on lightsail_clipboard

echo clean dead servers
pkill ncat

echo starting pbcopy server on 2224
ncat --keep-open --listen --sh-exec "pbcopy" 127.0.0.1 2224  &

echo starting pbpaste server on 2225
# ncat --keep-open --listen --sh-exec "pbpaste" 127.0.0.1 2225  &
ncat --keep-open --listen --sh-exec "pbpaste" 127.0.0.1 2225  &

echo starting SSH
ssh lightsail_clipboard /usr/bin/sleep 360000
echo Done ssh sleeping

