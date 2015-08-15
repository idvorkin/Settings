ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)" 
brew install brew-cask	cask		emacs		fish		libevent	openssl		tmux		vim ag
brew cask install google-chrome  iterm2	      karabiner	     seil	    skype

# shared git stetup

git config --global user.email "idvorkin@gmail.com"
git config --global user.name "Igor Dvorkin"
git config --global push.default simple

# in seil map caps -> F19 (Keycode: 80) 
ln ~/settings/private.xml ~/Library/Application\ Support/karabiner/private.xml
ln ~/settings/_vsvimrc ~/.vsvimrc 
ln ~/settings/private.xml  ~/.vimrc 
git clone https://github.com/gmarik/vundle.git ~/vimfiles/bundle/vundle

# 

