
# VM Settings
# 1) Enable Video Card Acceleration Settings -> Display
# 2) Enable shared clipboard
# 3) Enable directory sharing in a temp directory (Demiliatrized zone) 
# 4) Setup Pia Proxy
# 5) Insert Guest Disk and Install it.

# Version of node included is **old** so get the latest node 8.
# https://nodejs.org/en/download/package-manager/#debian-and-ubuntu-based-linux-distributions

curl -sL https://deb.nodesource.com/setup_8.x | sudo -E bash -
sudo apt-get install -y nodejs build-essentials.

sudo apt-get install virtualbox-guest-additions-iso vim-gnome vim git zsh ruby curl ruby-dev zlib1g-dev bittorrent silversearcher-ag git-gui tmux


# assume already cloned settings.
mkdir ~/gits
git clone https://github.com/idvorkin/settings ~/gits/settings
ln -s ~/settings ~/gits/settings
git clone https://github.com/idvorkin/idvorkin.github.io ~/gits/idvorkin.github.io


#Setup OhMyZSH
wget https://github.com/robbyrussell/oh-my-zsh/raw/master/tools/install.sh -O - | zsh

#setup Githubpages.
#https://help.github.com/articles/using-jekyll-with-pages/
sudo gem install bundler

cd ~/gits/idvorkin.github.io
bundle install

# Setup HomeBridge
# wink3 package - https://www.npmjs.com/package/homebridge-wink3
# homebridge package - https://github.com/nfarina/homebridge
