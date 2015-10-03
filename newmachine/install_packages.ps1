$packages = @"
git 
nodejs 
ag 
conemu 
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
"@.split()

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
