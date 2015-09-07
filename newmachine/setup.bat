
REM Setup dropbox Paths
        mklink /d \dropbox %USERPROFILE%\dropbox
        mklink /d \bin_drop %USERPROFILE%\dropbox\bin_drop

REM Setup Vim Paths
        mklink %USERPROFILE%\_vimrc c:\gits\settings\default_vimrc
REM Setup Mercurial Path
        mklink %USERPROFILE%\Mercurial.ini c:\hg\hg\Mercurial.ini

REM Setup Vim Paths
        mklink %USERPROFILE%\_vsvimrc c:\gits\settings\_vsvimrc

REM Setup Clink
        set TARGETFILE=clink_inputrc
        set TARGET=%USERPROFILE%\AppData\local\clink\%TARGETFILE%
        del  %TARGET%
        mklink %TARGET% c:\gits\settings\%TARGETFILE%

REM Setup Auto Hot Key Path
        del %USERPROFILE%\Documents\AutoHotkey.ahk 
        mklink %USERPROFILE%\Documents\AutoHotkey.ahk c:\hg\autohotkey\AutoHotkey.ahk

        del %USERPROFILE%\Documents\vim_onenote.ahk
        mklink %USERPROFILE%\Documents\vim_onenote.ahk c:\hg\autohotkey\vim_onenote.ahk

        del %USERPROFILE%\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1
        mklink %USERPROFILE%\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1 c:\gits\settings\Microsoft.PowerShell_profile.ps1

REM Setup choco stuff
    @powershell -NoProfile -ExecutionPolicy unrestricted -Command "iex ((new-object net.webclient).DownloadString('https://chocolatey.org/install.ps1'))" && SET PATH=%PATH%;%ALLUSERSPROFILE%\chocolatey\bin

    REM Install PSGet
    REM
    @powershell -NoProfile -ExecutionPolicy unrestricted -Command "iex (new-object Net.WebClient).DownloadString('http://psget.net/GetPsGet.ps1')"

REM Map Ctrl2Cap  -- More Info:  http://luvit.me/1MN7TCQ
    @powershell -NoProfile -ExecutionPolicy unrestricted -Command "Set-ItemProperty -path 'HKLM:\SYSTEM\CurrentControlSet\Control\Keyboard Layout' -name 'Scancode Map' -Value ([byte[]](0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x02,0x00,0x00,0x00,0x1d,0x00,0x3a,0x00,0x00,0x00,0x00,0x00))"

REM Shared with Mac/install.sh
git config --global user.email "idvorkin@gmail.com"
git config --global user.name "Igor Dvorkin"
git config --global push.default simpleM


REM test a current machine with choco list -localonly
    cinst git -y
    cinst nodejs -y
    cinst ag -y
    cinst conemu -y
    cinst gvim -y
    cinst nunit -y
    cinst nuget -y
    cinst repo -y
    cinst googlechrome -y
    cinst vim -y
    cinst linqpad -y
    cinst git-credential-winstore -y
    cinst nuget.commandline -y
    cinst autohotkey -y
    choco install visualstudio2015community -y
    choco install resharper -y
    cinst markdownpad2
    cinst windirstat -y
    choco install f.lux -y

REM Install Posh-Git
@powershell -NoProfile -ExecutionPolicy unrestricted -Command "Install-Module Posh-Git -force"

REM Install Repos I use.
cd /d c:\gits
git clone  https://github.com/VundleVim/Vundle.vim %USERPROFILE%/vimfiles/bundle/vundle
git clone https://github.com/idvorkin/onom
git clone  https://github.com/idvorkin/Vim-Keybindings-For-Onenote
git clone https://github.com/idvorkin/LinqpadSnippets
git clone https://github.com/idvorkin/linqpadDataExplore
git clone https://idvorkin@bitbucket.org/idvorkin/igor2.git