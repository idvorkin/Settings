let g:in_nvim=1

source ~/.vimrc

" Remap terminal keys, C-W, C-W leaves the terminal window
:tnoremap <C-W><C-W> <C-\><C-n>

" typing q will erase that buffer
augroup Terminal
  autocmd!
  autocmd TermOpen * nnoremap <buffer> q :bd<CR>
augroup END

" Reload when saving vimrc, which is the whole point of changing dotfile
augroup reload_nvimrc | au!
        au BufWritePost *init.vim source ~/settings/nvim/init.vim
        au BufWritePost *nvim_init.lua source ~/settings/nvim/init.vim
augroup END

function! RunInteractiveShellCommand(command)
  execute 'terminal ' . a:command
  startinsert
endfunction

command! -nargs=* Shell call RunInteractiveShellCommand(<q-args>)

" Remap  when in nvim
command! Sodot :so ~/settings/nvim/init.vim
command! Soed execute 'lcd ~/settings' | Telescope git_files
command! IgBlog execute 'lcd ~/gits/idvorkin.github.io' | Telescope git_files
cab gd :terminal export PAGER=don_t_use_me && git diff


" Configure lualine


:set shadafile=~/.nvim.shadafile

:nnoremap <C-P> :Telescope frecency<cr>
:nnoremap <C-O> :FF<cr>
:nnoremap <C-I> :Telescope aerial<CR>
set fillchars+=diff:╱


" Setup lua folding
syntax region luaFunction start="function" end="end" fold
syntax region luaBlock start="do" end="end" fold
autocmd FileType lua setlocal foldmethod=syntax

iab epbyt <esc>:lua InsertYouTubeTemplate()<CR>



" Common work flow: Switch to current  directory, git write, commit and push.
cab DdG  :lcd %:p:h<CR>:Gwrite<CR>:!git diff --staged %:p <CR> :!read -k <CR>:!git commit %:p <CR>:!git push<CR>
cab DdC :lua GitCommitAndPush()<CR>

:luafile ~/.config/nvim/nvim_init.lua
:luafile ~/.config/nvim/nvim_convo.lua
