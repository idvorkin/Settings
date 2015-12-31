# Size of packages by install size
dpkg-query -W --showformat='${Installed-Size;10}\t${Package}\n' | sort -k1,1n

# https://forums.virtualbox.org/viewtopic.php?t=15868

# Mount the host filesystem in ~/etc/rc.d
# mount -t vboxsf share /home/ubuntu/host
sudo mount -t vboxsf -o rw,uid=1000,gid=1000 share ~/host
