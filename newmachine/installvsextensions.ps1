#http://stackoverflow.com/questions/28652798/install-vs-net-extension-from-command-line

function Get-Batchfile ($file) {
  $cmd = "`"$file`" & set"
    cmd /c $cmd | Foreach-Object {
      $p, $v = $_.split('=')
        Set-Item -path env:$p -value $v
    }
}

function VsVars32()
{
    $BatchFile = join-path $env:VS140COMNTOOLS "vsvars32.bat"
    Get-Batchfile `"$BatchFile`"
}

function curlex($url, $filename) {
  $path = [io.path]::gettemppath() + "\" + $filename
  if( test-path $path ) { rm -force $path }
  (new-object net.webclient).DownloadFile($url, $path)

  return new-object io.fileinfo $path
}

function installsilently($url, $name) {
  echo "Installing $name"
  $extension = (curlex $url $name).FullName
  $result = Start-Process -FilePath "VSIXInstaller.exe" -ArgumentList "/q $extension" -Wait -PassThru;
}


# INSTALL VS Extenaions
installsilently http://visualstudiogallery.msdn.microsoft.com/59ca71b3-a4a3-46ca-8fe1-0e90e3f79329/file/6390/49/VsVim.vsix VsVim.vsix
installsilently https://visualstudiogallery.msdn.microsoft.com/f1cc3f3e-c300-40a7-8797-c509fb8933b9
