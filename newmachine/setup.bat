
REM Setup dropbox Paths
        mklink /d \dropbox %USERPROFILE%\dropbox
        mklink /d \bin_drop %USERPROFILE%\dropbox\bin_drop

REM Setup Vim Paths
        mklink %USERPROFILE%\_vimrc c:\gits\settings\default_vimrc
REM Setup Mercurial Path
        mklink %USERPROFILE%\Mercurial.ini c:\hg\hg\Mercurial.ini

REM Setup Vim Paths
        mklink %USERPROFILE%\_vsvimrc c:\gits\settings\_vsvimrc

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

REM Install PS-Get
@powershell -NoProfile -ExecutionPolicy unrestricted -Command "Install-Module Posh-Git –force"
