$packages = @"
googlechrome
nodejs
"@.split()

# no longer installed:
# slack

# Drawing  Tools
$packages += "paint.net"

#VM Management
$packages += @"
virtualbox
vagrant
"@.split()

#Misc windows utilities
$packages += @"
windirstat
procexp
handle
7zip
"@.split()

# Windows automation.
# Wox is like alfred, press A+Space to run a launcher.
# Also add switcheroo to jump to active window.

$packages += @"
autohotkey
wox
"@.split()

# Snipping tool
$packages += "sharex"

# Dim screen brightness in the evening.
$packages += "flux"

# Diagnostic utilities
$packages += @"
hdtune
cpu-z
"@.split()

#Command Line tooling
$packages += @"
visualstudiocode
vim
ctags
ag
ripgrep
fzf
plantuml
"@.split()

#Git tools
$packages += @"
git
sourcetree
"@.split()


#dot net development
# Other good packages (NCrunch)
$packages += @"
nuget.commandline
nunit
nuget
linqpad
"@.split()


$java_install_packages_manually += @"
maven
intellijidea-community
"@.split()

# These packages are often installable, but something is goofy right now with them.
$install_these_packages_manually_for_now += @"
visualstudio2017community
eclipse-java-neon
"@.split()

# resharper  -- DO NOT INSTALL RESHARPER, as only have license for resharper 9, and so far 10 isn't good enough better.

# Remote Assistance
$packages += "teamviewer"

# programming language python and ruby.
# gem install activesupport github-pages wdm jekyll bundler
$packages += "
anaconda3
ruby
ruby2.devkit
@".split()

#Video Editor
$packages += "shotcut"

# DVD -> ISO
$packages += @"
makemkv
handbrake
imagemagick
youtube-dl
vlc
"@.split()

# Network tools
$packages += @"
nmap
wget
curl
"@.split()

# iphone /ios not actually in chocolatey.
# CopyTrans

# Ebooks
$packages += "calibre"
#
# Writing tools
$packages += "vale"

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
