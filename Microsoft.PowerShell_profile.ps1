Push-Location (Split-Path -Path $MyInvocation.MyCommand.Definition -Parent)

# ps-readline for vi support in powershell.
Import-Module PSReadLine  
Set-PSReadlineOption -EditMode vi

# git support
Import-Module posh-git
# jump to cached path.
Import-Module z


# Built-in, default PowerShell prompt w posh-get extension.
$GitPromptSettings.DefaultPromptSuffix = '`n$(''>'' * ($nestedPromptLevel + 1)) '

Pop-Location

function Convert-Mov-To-Mp4 ($infile, $outfile) {
    ffmpeg -i $infile -qscale 0 $outfile
}

function Restart-Explorer {
    Get-Process | where name -eq explorer  |kill ; explorer
    Add-Type -Name ConsoleUtils -Namespace WPIA -MemberDefinition @'
    [DllImport("user32.dll")]
            public static extern int FindWindow(string lpClassName,string lpWindowName);
            [DllImport("user32.dll")]
            public static extern int SendMessage(int hWnd, uint Msg, int wParam, int lParam);
                
            public const int WM_SYSCOMMAND = 0x0112;
            public const int SC_CLOSE = 0xF060;
'@
    # When explorer restarts, it opens a new file explorer window - wait a few seconds for it to pop up before closing it.
    for ($i = 0; $i -lt 10 ; $i++)
    {
        start-sleep -s 1
        [int]$handle = [WPIA.ConsoleUtils]::FindWindow('CabinetWClass','File Explorer')
        if ($handle -gt 0)
        {
            [void][WPIA.ConsoleUtils]::SendMessage($handle, [WPIA.ConsoleUtils]::WM_SYSCOMMAND, [WPIA.ConsoleUtils]::SC_CLOSE, 0)
            break;
        }  
    }
}
function Kill-GitGui {Get-Process | where name -eq wish  |kill }
function Zach-Age {((get-date) - (get-date 4/22/2010)).TotalDays/7}
function Flatten-Big-Files {
    # downloaded movies tend to nest subdirectories and have junk, pull files down to parent director
    # 1) Recurse subdirectories
    # 2) Move movies files to root
    gci -R | ? {$_.Length -gt 10mb -and -not ($_.IsContainer) } | % {Move-Item $_.PSPath . }
}

# Chocolatey profile
$ChocolateyProfile = "$env:ChocolateyInstall\helpers\chocolateyProfile.psm1"
if (Test-Path($ChocolateyProfile)) {
  Import-Module "$ChocolateyProfile"
}

# Helpful jekyll commands.
function Jekyll-Serve {Start-Process bundle "exec jekyll serve --incremental --drafts"} 
function Jekyll-Build {bundle exec jekyll build --drafts}

# Helpful recipes
################################################
#
# Copy all files ending in .mardown to .md
#   ls *markdown | % {move "$($_.BaseName).markdown" "$($_.BaseName).md"}
