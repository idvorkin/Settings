
# VM Settings
# 1) Enable Video Card Acceleration Settings -> Display
# 2) Enable shared clipboard
# 3) Enable directory sharing in a temp directory (Demiliatrized zone) 
# 4) Setup Pia Proxy
# 5) Insert Guest Disk and Install it.


# assume already cloned settings.
sudo apt-get install virtualbox-guest-additions-iso vim-gnome vim git zsh ruby curl ruby-dev zlib1g-dev nodejs qbittorrent
mkdir ~/gits
ln -s ~/settings ~/gits/settings
git clone https://github.com/idvorkin/idvorkin.github.io ~/gits/idvorkin.github.io

#Setup OhMyZSH
wget https://github.com/robbyrussell/oh-my-zsh/raw/master/tools/install.sh -O - | zsh

#setup Githubpages.
#https://help.github.com/articles/using-jekyll-with-pages/
sudo gem install bundler

cd ~/gits/idvorkin.github.io
bundle install


# Misc 
