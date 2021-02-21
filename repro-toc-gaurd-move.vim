syntax  on

function! SetupPlugins()
"  Vundle setup
    " Instructions @  https://github.com/VundleVim/Vundle.vim/blob/master/README.md
    " git clone https://github.com/VundleVim/Vundle.vim %USERPROFILE%/vimfiles/bundle/vundle
    " MAC - $ git clone https://github.com/VundleVim/Vundle.vim.git ~/.vim/bundle/Vundle.vim
    filetype off
    set rtp+=~/vimfiles/bundle/Vundle
    set rtp+=~/.vim/bundle/Vundle.vim


    call vundle#begin()

    " Markdown stuff
    " *******************************
    Bundle 'mzlogin/vim-markdown-toc'
    " :GenTocGFM
    " :UpdateToc

    Bundle 'plasticboy/vim-markdown'
    " :TOC - Generate a toc sidebar
    " :VSize - Resize to 20
    " ]] Next Header
    " [[ Prev Header
    call vundle#end()            " required
endfunc

call SetupPlugins()

filetype plugin indent on    " required

let g:vim_markdown_follow_anchor = 1 " ge will jump to anchor (TBD: Map to C-]
let g:vim_markdown_toc_autofit = 1 " Great for not wasting extra space
let g:vim_markdown_frontmatter = 1
set conceallevel=2
let g:vim_markdown_new_list_item_indent = 0
let g:vim_markdown_folding_level = 4
let g:vmt_list_item_char='-'

" Always enable softpencil

"By default, swap out all instances in a line during substitutions
set gdefault
"
" vim:foldmethod=indent:
