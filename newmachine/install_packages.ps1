$packages = @"
git 
nodejs 
ag 
nunit 
nuget 
repo 
googlechrome 
vim 
linqpad 
git-credential-winstore 
nuget.commandline 
autohotkey 
visualstudio2015community 
resharper 
markdownpad2 
windirstat 
slack 
f.lux 
procexp 
paint.net 
clink
sharex
ctags
skype
github
sourcetree
calibre
pretzel
"@.split()

# Make sure conemu is the last thing, as it needs to close the window.
$packages += "conemu"

foreach ($package in $packages | ? {$_ -ne ""}) 
{
    echo "Installing: $package" 
    cinst  -y $package
}

foreach ($package in $packages | ? {$_ -ne ""}) 
{
    echo "Installing: $package" 
    cup  -y $package
}
