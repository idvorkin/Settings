sudo apt-get install virtualbox-guest-additions-iso vim-gnome vim git zsh ruby curl ruby-dev zlib1g-dev
cd ~
git clone https://github.com/idvorkin/settings
mkdir gits
git clone https://github.com/idvorkin/idvorkin.github.io

#Setup OhMyZSH
wget https://github.com/robbyrussell/oh-my-zsh/raw/master/tools/install.sh -O - | zsh

#setup Githubpages.
#https://help.github.com/articles/using-jekyll-with-pages/
sudo gem install bundler

cd ~/gits/idvorkin.github.io
bundle install
