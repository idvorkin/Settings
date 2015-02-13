
REM Setup dropbox Paths
        mklink /d \dropbox %USERPROFILE%\dropbox
        mklink /d \bin_drop %USERPROFILE%\dropbox\bin_drop

REM Setup Vim Paths
        mklink %USERPROFILE%\_vimrc c:\gits\settings\default_vimrc
REM Setup Mercurial Path
        mklink %USERPROFILE%\Mercurial.ini c:\hg\hg\Mercurial.ini

REM Setup Auto Hot Key Path
        del %USERPROFILE%\Documents\AutoHotkey.ahk 
        mklink %USERPROFILE%\Documents\AutoHotkey.ahk c:\hg\autohotkey\AutoHotkey.ahk
        del %USERPROFILE%\Documents\vim_onenote.ahk
        mklink %USERPROFILE%\Documents\vim_onenote.ahk c:\hg\autohotkey\vim_onenote.ahk

REM Setup choco stuff
    @powershell -NoProfile -ExecutionPolicy unrestricted -Command "iex ((new-object net.webclient).DownloadString('https://chocolatey.org/install.ps1'))" && SET PATH=%PATH%;%ALLUSERSPROFILE%\chocolatey\bin

REM test a current machine with choco list -localonly
    cinst git
    cinst nodejs
    cinst ag
    cinst conemu
    cinst gvim
    cinst nunit
    cinst nuget
    cinst repo
