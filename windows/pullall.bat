powershell -Command "& {gci c:\gits -Directory | %% {cd $_.FullName; echo $_.FullName ; git pull}}"
