# Sigh, sometimes you need to fix openssl when you get SSL errors
# brew reinstall openssl@1.1
# pip3 install setuptools_rust
brew unlink python@3.10 && brew link --overwrite python@3.10
