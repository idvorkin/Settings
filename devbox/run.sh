# docker:build": "docker build -t devbox .
# docker run -v ~/blog:/root/blog -it -P -p 127.0.0.1:4000:4000 devbox
# docker run -v ~/blog:/root/blog -it -P -p 127.0.0.1:4000:4000 devbox
 docker run \
     -v devbox_vol_usr:/usr \
     -v devbox_vol_var:/var \
     -v devbox_vol_etc:/etc \
     -v devbox_vol_root:/root \
     -v ~/blog:/root/blog\ \
     -it -P -p 127.0.0.1:4000:4000 devbox
