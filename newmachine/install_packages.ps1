$packages = @"
googlechrome 
skype
slack 
nodejs 
repo 
"@.split()

# Drawing  Tools
$packages += "paint.net"

# Static Blog and markdown (Jekyll for windows)
$packages += @"
pretzel
haroopad
markdownpad2 
"@.split()

#VM Management
$packages += @"
virtualbox
vagrant
"@.split()

#Misc windows utilities
$packages += @"
autohotkey 
windirstat 
procexp 
wget
curl
"@.split()

# Snipping tool
$packages += "sharex"

# Dim screen brightness in the evening.
$packages += "flux"

#Command Line tooling 
$packages += @"
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
"@.split()

# resharper  -- DO NOT INSTALL RESHARPER, as only have license for resharper 9, and so far 10 isn't good enough better.

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
