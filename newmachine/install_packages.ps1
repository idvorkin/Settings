$packages = @"
googlechrome 
skype
slack 
nodejs 
repo 
markdownpad2 
virtualbox
"@.split()

# Drawing  Tools
$packages += "paint.net"

# Static Blog (Jekyll for windows)
$packages += "pretzel"

#Misc windows utilities
$packages += @"
autohotkey 
windirstat 
procexp 
"@.split()

# Snipping tool
$packages += "sharex"

# Dim screen brightness in the evening.
$packages += "flux"

#Command Line tooling 
$packages += @"
clink
vim 
ctags
ag 
"@.split()

#Git tools
$packages += @"
git 
git-credential-winstore 
sourcetree
"@.split()


#dot net development
$packages += @"
nuget.commandline 
nunit 
nuget 
linqpad 
visualstudio2015community 
resharper 
"@.split()

# Remote Assistance
$packages += "teamviewer"

#Video Editor
$packages += "shotcut"

# DVD -> ISO
$packages += "makemkv"

# Ebooks
$packages += "calibre"

# Windows Terminal Replacement
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
