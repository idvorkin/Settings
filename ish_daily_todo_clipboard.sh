#

echo  Running copy to clipboard on lightsail

ssh lightsail /home/ec2-user/homebrew/bin/python3 /home/ec2-user/gits/linqpadsnippets/python/dump_grateful.py todo  --text-only 1 | tee /dev/clipboard
echo Done ssh sleeping


