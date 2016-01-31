# http://stackoverflow.com/questions/1276703/how-to-make-zsh-run-as-a-login-shell-on-mac-os-x-in-iterm
#
echo Copy below line to paste on plugics
echo plugins=\(git osx lol quote vi-mode web-search wd\)
echo bindkey -v 
read
vi  ~/.zshrc
echo copy below line to add to /etc/shells
which zsh
read
sudo vim /etc/shells

echo change shell
sudo chsh -s $(which zsh)

