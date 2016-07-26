REM First Run As Admin - Install Chocolatey
    @powershell -NoProfile -ExecutionPolicy unrestricted -Command "iex ((new-object net.webclient).DownloadString('https://chocolatey.org/install.ps1'))" && SET PATH=%PATH%;%ALLUSERSPROFILE%\chocolatey\bin
REM Second Run As Admin - Install Git
    cinst git -y
REM Third Run As Admin 
    mkdir c:\gits\ && cd /d c:\gits && git clone https://github.com/idvorkin/settings
REM Fourth Run As Admin -- c:\gits\settings\newmachine\setup.bat


REM Setup dropbox Paths
        mklink /d \dropbox %USERPROFILE%\dropbox
        mklink /d \bin_drop %USERPROFILE%\dropbox\bin_drop

REM Setup Vim Paths
        mklink %USERPROFILE%\_vimrc c:\gits\settings\default_vimrc
REM Setup Mercurial Path
        mklink %USERPROFILE%\Mercurial.ini c:\hg\hg\Mercurial.ini

        mklink %USERPROFILE%\_vsvimrc c:\gits\settings\_vsvimrc
        mklink %USERPROFILE%\.ctagsrc c:\gits\settings\shared\ctags

REM GITS directory alias
        mklink /d %USERPROFILE%\gits c:\gits

REM Setup Pull All
        set TARGETFILE=pullall.bat
        set TARGET=c:\gits\%TARGETFILE%
        del  %TARGET%
        mklink %TARGET% c:\gits\settings\newmachine\%TARGETFILE% 

REM Setup Clink


        del %USERPROFILE%\_inputrc 
REM clink doesn't yet support links, so set this as a copy for now.
        copy c:\gits\settings\clink_inputrc  %USERPROFILE%\_inputrc 

REM Setup Auto Hot Key Path
        del %USERPROFILE%\Documents\AutoHotkey.ahk 
        mklink %USERPROFILE%\Documents\AutoHotkey.ahk c:\hg\autohotkey\AutoHotkey.ahk

        del %USERPROFILE%\Documents\vim_onenote.ahk
        mklink %USERPROFILE%\Documents\vim_onenote.ahk c:\hg\autohotkey\vim_onenote.ahk

        del %USERPROFILE%\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1
        mklink %USERPROFILE%\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1 c:\gits\settings\Microsoft.PowerShell_profile.ps1

REM Setup choco stuff

    REM Install PSGet
    REM
    @powershell -NoProfile -ExecutionPolicy unrestricted -Command "iex (new-object Net.WebClient).DownloadString('http://psget.net/GetPsGet.ps1')"

REM Map Ctrl2Cap  -- More Info:  http://luvit.me/1MN7TCQ
    @powershell -NoProfile -ExecutionPolicy unrestricted -Command "Set-ItemProperty -path 'HKLM:\SYSTEM\CurrentControlSet\Control\Keyboard Layout' -name 'Scancode Map' -Value ([byte[]](0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x02,0x00,0x00,0x00,0x1d,0x00,0x3a,0x00,0x00,0x00,0x00,0x00))"

REM Shared with Mac/install.sh
REM Useful, write current linspect to the command line
REM <linespec>w !bash
git config --global user.email "idvorkin@gmail.com"
git config --global user.name "Igor Dvorkin"
git config --global push.default simple
git config --global alias.co checkout
git config --global alias.com "checkout master"
git config --global alias.br branch
git config --global alias.ci commit
git config --global alias.st status
git config --global alias.logc log master..
git config --global alias.logp "log --pretty=format:'%C(yellow)%h%Cred%d %Creset%s %C(yellow)[%cn] %C(green)(%ar)' --decorate"

REM Setup policy execution policy
powershell Set-ExecutionPolicy RemoteSigned

REM test a current machine with choco list -localonly
powershell .\install_packages.ps1

REM Install Posh-Git
@powershell -NoProfile -ExecutionPolicy unrestricted -Command "Install-Module Posh-Git -force"

REM To view git branch from clink 
REM gitshowbranch /i

REM Install Repos I use.
git clone  https://github.com/VundleVim/Vundle.vim %USERPROFILE%/vimfiles/bundle/vundle
git clone https://github.com/idvorkin/onom
git clone https://github.com/idvorkin/Vim-Keybindings-For-Onenote
git clone https://github.com/idvorkin/LinqpadSnippets
git clone https://github.com/idvorkin/linqpadDataExplore
git clone https://github.com/idvorkin/idvorkin.github.io
git clone https://idvorkin@bitbucket.org/idvorkin/igor2.git

