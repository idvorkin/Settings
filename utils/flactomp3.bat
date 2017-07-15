" Might only work outside a command line.
for %f in (*.flac) do ffmpeg -y -i "%~nf.flac" -q:a 0  "%~nf".mp3
