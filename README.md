# Settings
A Place to store my settings/dotFiles/etc

## Normal linux

Mostly done via script, contained here:

    cd ~
    git clone https://github.com/idvorkin/settings


## Alpine Linux (using iSH)


I use ish as my ssh client, with some minor tweaks:

    cd ~
    apk add git vim openssh-client tig ranger zsh
    git clone https://github.com/idvorkin/settings
    ln -s ~/settings/shared/ssh_config ~/.ssh/config

## Windows

1) Install chocolatey (new admin window)

    @powershell -NoProfile -ExecutionPolicy Bypass -Command "iex ((new-object net.webclient).DownloadString('https://chocolatey.org/install.ps1'))" && SET PATH=%PATH%;%ALLUSERSPROFILE%\chocolatey\bin

2) Install git (new admin window)

    cinst git

  3) Clone settings (new admin window)

    cd \
    git clone https://github.com/idvorkin/settings


