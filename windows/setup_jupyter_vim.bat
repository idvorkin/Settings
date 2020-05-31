set NB_HOME= %USERPROFILE%\AppData\Roaming\jupyter

mkdir %NB_HOME%\nbextensions
cd %NB_HOME%\nbextensions   
git clone https://github.com/lambdalisue/jupyter-vim-binding vim_binding 
jupyter nbextension enable vim_binding/vim_binding
