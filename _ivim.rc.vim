" Common  things in all of my vimrc's
"  File in git: ~/gits/settings/default_vimrc
" set filetype=vim
set nocompatible
set nohls
set ignorecase
set noincsearch
set guioptions-=T
set guioptions-=m
" Remove right hand scroll bars
set guioptions-=R
set guioptions-=r
set sw=4
set ts=4
set expandtab
set cindent
set laststatus=2
runtime macros/matchit.vim
syntax  on
:ca lcdb lcd %:p:h

set tags=tags;/


" OMG - How to support italics in vim in TMUX
" https://rsapkf.netlify.com/blog/enabling-italics-vim-tmux

function! FixCallStacks()
    :%s;\[NLN\];\r;g
    :%s;\[TAB\];\t;g
endfunction

function! StripPerfCounterTags()
    %s;\(\\.*\)<.*>;\1;
    %s;<.*>;;
endfunction

function! FixMojiBake()
    " See charector under cursor -
    "   ga
    " Search for non unicode charecters
    " /[^\x00-\x7F]

    " Replace hypens
    :%s/\%x97/-/g
    :%s/\%x96/-/g

    " Replace smart quotes
    :%s/\%x93/"/g
    :%s/\%x94/"/g

    " Remove TMs and oes
    :%s/\%x99//g
    :%s/\%x9c//g
    :%s/\%x9d//g
    "

    " Replace apostrophe
    :%s/\%x92/'/g
    " Replace apostrophe in bear
    :%s/\%xe2\%x80/'/g
endfunction

function! FirstPersonToThirdPerson()

    " Trim the whitespace off the ends
    :%s;\s\+$;;

    " Add a period to the end if it's not there.
    :g!/\.$/s;$;.;

    " do a bunch of replaces.
    :%s;^I;You;
    :%s;\. ^I;You;g
    :%s; I ; you ;g
    :%s; am ; are ;g
    :%s; me\([ \.,$]\); you\1;g
    :%s; me\([ \.,$]\); you\1;g
    :%s; mine\([ \.,$]\); yours\1;g
    :%s;^Me ;You ;g
    :%s; myself\([ \.,$]\); yourself\1;g
    :%s; my ; your ;g
    :%s;^My ;Your ;g
    :%s;^\.$;
endfunction

function! SetupPlugins()
"  Vundle setup
    " Instructions @  https://github.com/VundleVim/Vundle.vim/blob/master/README.md
    " git clone https://github.com/VundleVim/Vundle.vim %USERPROFILE%/vimfiles/bundle/vundle
    " MAC - $ git clone https://github.com/VundleVim/Vundle.vim.git ~/.vim/bundle/Vundle.vim
    filetype off
    set rtp+=~/vimfiles/bundle/Vundle
    set rtp+=~/.vim/bundle/Vundle.vim

    if filereadable(expand("~/homebrew/opt/fzf/plugin/fzf.vim"))
        so ~/homebrew/opt/fzf/plugin/fzf.vim
        " set rtp+=/usr/local/opt/fzf/plugin/
    endif

    call vundle#begin()

    Bundle 'VundleVim/Vundle.vim'
    Bundle 'webdevel/tabulous'
    Bundle 'junegunn/limelight.vim'
    " Bundle 'reedes/vim-pencil'
    Bundle 'junegunn/goyo.vim'
    " Bundle 'inkarkat/vim-spellcheck'
    " :IndentGuidesEnable
    " :IndentGuidesDisable
    " :IndentGuidesToggle
    Bundle 'nathanaelkane/vim-indent-guides'

    Bundle 'mhinz/vim-startify'
    Bundle 'tpope/vim-surround'
    Bundle 'bling/vim-airline'
    Bundle 'chrisbra/csv.vim'
    Bundle 'altercation/vim-colors-solarized'
    Bundle 'vim-scripts/ZoomWin'
    "
    " change font size \\++
    Bundle 'drmikehenry/vim-fontsize'


    " Markdown stuff
    " *******************************
    Bundle 'godlygeek/tabular'
    Bundle 'mzlogin/vim-markdown-toc'
    " :GenTocGFM
    " :UpdateToc

    Bundle 'plasticboy/vim-markdown'
    " :TOC - Generate a toc sidebar
    " :VSize - Resize to 20
    " ]] Next Header
    " [[ Prev Header
    Bundle 'parkr/vim-jekyll'
    Bundle 'christoomey/vim-quicklink'

    Bundle 'elzr/vim-json'
    Bundle 'PProvost/vim-ps1'
    Bundle 'miyakogi/conoline.vim'
    Bundle 'othree/javascript-libraries-syntax.vim'
    Bundle "pangloss/vim-javascript"
    Bundle "scrooloose/nerdtree"
    Bundle "rking/ag.vim"
    Bundle "OrangeT/vim-csharp.git"
    " OmniSharp doesn't seem to wrok on WSL, let it go.
    " Bundle "OmniSharp/omnisharp-vim"
    Bundle "lukaszkorecki/workflowish"
    Bundle "keith/swift.vim"
    " http://vimcolors.com/?page=11
    Bundle 'flazz/vim-colorschemes'
    Bundle 'dhruvasagar/vim-table-mode'
    Bundle 'atelierbram/vim-colors_atelier-schemes'
    " Comment \cc
    " Uncomment \cu
    Bundle 'scrooloose/nerdcommenter'
    Bundle 'HerringtonDarkholme/yats.vim'

    " Plugin 'plytophogy/vim-virtualenv'
    " black needs virtual env, which can't find
    " Plugin 'ambv/black'
    " Write on Save
    " autocmd BufWritePost *.py execute ':Black'
    call vundle#end()            " required
endfunc

call SetupPlugins()


func! SetupAutoFixEnglish()
    " Can fix with Sed
    " sed -i -e 's/ teh / the /g' *
    " grab these from .aspell.en.prepl
     :iab teh the
     :iab hte the
     :iab taht that
     :iab ot to
     :iab hte the
     :iab zach Zach
     :iab alot a lot
     :iab amelia Amelia
     :iab tori Tori
     :iab ammon Ammon
     :iab adn and
     :iab nad and
     :iab cna can
     :iab dont don't
     :iab   fi if
     :iab   fo of
     :iab iwll will
     :iab wiht with
    :iab htgorugh through
    :iab htis this
    :iab htourhg through
endfunc

call SetupAutoFixEnglish()



filetype plugin indent on    " required

" Set font-size default to be a decent size.
if ( has("win32") || has("win64") || has("win16") )
    if !exists("g:loaded")
        " TBD: Setting the guifont causes the window to move, so only set the
        " guifont if it hasn't been set before.
        let defaultFont="Consolas:h14"
        let g:loaded=1
        exec "set guifont=".defaultFont
    endif
endif
"
"WOFL Helpers
" HELP @ https://github.com/lukaszkorecki/workflowish"

func! OneNoteToWafl()
    " Tabs to spaces
    %s;\t;  ;g
    " Dots to *'s
    %s/\%u2022/*/g
    %s/\%u25cb/*/g
    %s/\%u00b7/*/g
endfunc

func! WriteOn()
    :Goyo
    :PencilSoft
    :Limelight
    " :ALEDisable " Remove spelling and grammer
endfunc
command! IGWriteOn :call WriteOn()
command! IGWriteOff :call WriteOff()

func! WriteOff()
    :Goyo
    :PencilOff
    :Limelight!
    " :ALEEnable "Remove spelling and grammer
endfunc


" TBD  Checkbox processing
"   add these only to markdown/html; checkboxopen, checkboxdone
"   Perhaps be clever and alternate between open and done like in OneNote via C-1
" Using digraphs - so cool
" C-K  9744 // Checkmark complete
" C-K  9745 // Checkmark open
" https://www.fileformat.info/info/unicode/char/2611/index.htm
" :dig td 9744
" :dig tc 9745

:iab mdt ☐
:iab mdtd ☐
:iab mdc ☑
:iab mdtodo ☐
:iab mddone  ☑
:cab mdcbt g/^#/s;# \*\*\(.*\)\*\*;# \1;

func! MarkDownWordToLink()
    " cool yiw takes the word, and jumps you to the beginning of it.
    normal yiwi[ea](http://www.pa.com)
endfunc

func! MarkDownClearBoldTitles()
":cab mdcbt g/^#/s;# \*\*\(.*\)\*\*;# \1;"
endfunc


func! MarkDownWordToGoogleLucky()
    " cool yiw takes the word, and jumps you to the beginning of it.
    normal yiwi[ea](http://www.google.com/search?btnI&q=)hp
endfunc

func! JsToSingleLineClipBoard()
    " Creating a bookmarklet requires all JS to be on a single line.
    " Make it a single line and put it on global clipboard to paste into JS
    " console.

    " NOTE: when creating bookmarklets, you'll need to terminate all lines
    " with a ';' and only use inline comments, and end with void();

    " NOTE: when making it the bookmark, you need to start with javascript:

    " Join all to one line.
    exec ":%j"
    " Copy to Clipboard
    normal "*yy
    " Undo
    normal u
    `
endfunc

if $TERM_PROGRAM =~ "iTerm"
    " No idea what these first two things are for - seem to be something
    " only needed in tmux (as opposed to just iTerm) - Groan, what a mess.
    " http://www.linuxquestions.org/questions/slackware-14/tip-24-bit-true-color-terminal-tmux-vim-4175582631/
    set termguicolors
endif

" Assume no longer need these - put back if you do.
let &t_8f = "\<Esc>[38;2;%lu;%lu;%lum"
let &t_8b = "\<Esc>[48;2;%lu;%lu;%lum"
set termguicolors

if has ("gui-running")
    :colo Tomorrow-Night-Blue
else
    :colo darkblue
endif

" Remove trailing whitespace:
autocmd BufWritePre * :%s/\s\+$//e

" Markdown
"  ToC - Sidebar to navigate.
"  ][  - sibling prev
"  []  - sibling next

let g:vim_markdown_follow_anchor = 1 " ge will jump to anchor (TBD: Map to C-]
let g:vim_markdown_toc_autofit = 1 " Great for not wasting extra space
let g:vim_markdown_frontmatter = 1
set conceallevel=2
let g:vim_markdown_new_list_item_indent = 0
let g:vim_markdown_folding_level = 4
let g:vmt_list_item_char='-'

" Always enable softpencil

so ~/.vim/bundle/vim-pencil/plugin/pencil.vim
so ~/.vim/bundle/goyo.vim/plugin/goyo.vim

autocmd BufEnter *md :exe ":SoftPencil"
" autocmd BufEnter *md :exe ":Limelight"
" autocmd BufLeave *md :exe ":Limelight!"
" let g:vim_markdown_folding_disabled = 1

" Break lines on word boudnaries only
set linebreak


" Jekyll  - posts for use with JPost
" JVPost -  create jekyll post in new vertical split
let g:jekyll_post_extension = '.md'
    let g:jekyll_post_template =  [
      \ '---',
      \ 'layout: post',
      \ 'title: "JEKYLL_TITLE"',
      \ 'date: "JEKYLL_DATE"',
      \ 'tags:',
      \ '  - ',
      \ '---',
      \ ]


let g:jekyll_post_dirs = ['_posts', '../_posts','_drafts','../_drafts']


function! EscapeKey()
    " Do mappings for funny keyboard
    :inoremap ` <esc>
    :inoremap C-` `
    :cnoremap ` <esc>
    :cnoremap C-` `
endfunction

function! FixEscapeKey()
    :call EscapeKey()
endfunction

let g:ale_fixers = {
            \   'javascript': ['prettier'],
            \   'css': ['prettier'],
            \   'markdown': ['prettier'],
            \}

" let g:ale_fix_on_save = 1
" Scripting reference
" https://devhints.io/vimscript
command! IGDaily :py3 MakeDailyPage()
command! IGYesterday :py3 MakeDailyPage(-1)
command! IGWeekly :py3 MakeWeeklyReport()
cab Ddt !python3 ~/gits/linqpadsnippets/python/dump_grateful.py todo
cab Ddg !python3 ~/gits/linqpadsnippets/python/dump_grateful.py grateful
cab Dda !python3 ~/gits/linqpadsnippets/python/dump_grateful.py awesome
command! Sodot :so ~/.vimrc
command! Soed :n ~/settings/default_vimrc
command! VSize :vert resize 20
command! IgTodo :r !python3 ~/gits/linqpadsnippets/python/dump_grateful.py todo 2
command! IgTD :lcd ~/gits/techdiary/<bar>:GFiles
command! Ig2 :lcd ~/gits/igor2/<bar>:GFiles
command! IgBlog :lcd ~/gits/idvorkin.github.io/<bar>:GFiles


" Common work flow: Switch to current directory, git write, commit and push.
cab DdC :lcd %:p:h<CR>:Gwrite<CR>:!git commit %:p -m "Save %"<CR>:!git push<CR>


" I never use ToHTML and it makes it harder to use Toc
" This is loaded in the html plugin, which I'm disabling.
" search for the below string to find it.
let g:loaded_2html_plugin = 'vim8.1_v1'
" leaving the below in for an example of one time load only
if  !exists ("g:execute_on_first_load_only")
    " Can only delete it once
    let  g:execute_on_first_load_only = "1"
    ":delc TOhtml
endif

"Auto spell check in aspell.
:cab aspell :w!<CR>:!aspell check %<CR>:e! %<CR>

" I always want the local cd.
:cab Gcd  :Glcd

"  Add alias to regenerate tags for this repo.
"  Using FD honors the .gitignore
:cab GTag  :Glcd<CR>:!ctags `fd md$`

:cab lg  :!lazygit


" Copied from https://gist.github.com/jackkinsella/aa7374a6832cca8a09eadc3434a33c24`
" Automatically reload file when underlying files change (e.g. git)
set autoread


"By default, swap out all instances in a line during substitutions
set gdefault


call FixEscapeKey()

function! SetupiPlug()
    iplug add 'https://github.com/junegunn/goyo.vim'
    iplug add 'https://github.com/reedes/vim-pencil'
    iplug add 'https://github.com/inkarkat/vim-spellcheck'
    iplug add 'https://github.com/tpope/vim-surround'
    iplug add 'https://github.com/parkr/vim-jekyll'
    iplug add "https://github.com/scrooloose/nerdtree"
    iplug add 'https://github.com/scrooloose/nerdcommenter'
    iplug add 'https://github.com/majutsushi/tagbar'
    iplug update
endfunc

function! SetupTagbar()
    " Update markdown tagbar need to install mdctags                                                                                                                                " https://github.com/wsdjeg/mdctags.rs#installation
    let g:tagbar_left=1
    let g:tagbar_sort = 0
    let g:tagbar_foldlevel = 99
    let g:tagbar_autoshowtag = 1
    let g:tagbar_compact = 1

    command! TagbarV let g:tagbar_vertical=1|:TagbarOpen
    command! TagbarH let g:tagbar_vertical=0|:TagbarOpen
:nnoremap <silent> <C-Y>  :TagbarOpen fj<cr>

endfunc
call SetupTagbar()



" Turn a line of ; into gratefulness line.

" Turn a line to grateful to avoid lots of escapes. Format
" <grateful for>;< god for>;<other>;<other for>;<igor for>
" Discovered Bands;Easy to use weights everywhere; Amazon ; Selling everything ; Using the bands
cab grl :s/\"*\s*\(.*\)\s*;\s*\(.*\);\s*\(.*\)\s*;\s*\(.*\)\s*;\s*\(.*\)/1. \1 **God** \2 **\3** \4 **Igor** \5<CR>
"

function! EscapeKey()
    " Do mappings for funny keyboard
    :inoremap ` <esc>
    :inoremap C-<esc> `
    :cnoremap ` <esc>
    :cnoremap C-<esc> `
endfunction

function! FJEscape()
    " Do mappings for funny keyboard
   :inoremap fj <esc>
   :cnoremap fj <esc>
endfunction
" :iunmap fj
" :cunmap fj
call FJEscape()

:inoremap <S-Del> <Del>

function! FixEscapeKey()
    :call EscapeKey()
endfunction
"
" Reload when saving vimrc, which is the whole point of changin dotfile
augroup reload_vimrc | au!
        au BufWritePost *default_vimrc source ~/.vimrc
        au BufWritePost *_ivim.rc.vim source ~/.vimrc
augroup END
set viminfo='1000,<50,s10,h
" vim:foldmethod=indent:
