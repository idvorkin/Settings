
let g:in_nvim=1


" Monkey patch startify to use nvim's dev icons
lua << EOF
function _G.webDevIcons(path)
  local filename = vim.fn.fnamemodify(path, ':t')
  local extension = vim.fn.fnamemodify(path, ':e')
  return require'nvim-web-devicons'.get_icon(filename, extension, { default = true })
end
EOF

function! StartifyEntryFormat() abort
  return 'v:lua.webDevIcons(absolute_path) . " " . entry_path'
endfunction

source ~/.vimrc



" Remap terminal keys, C-W, C-W leaves the terminal window
:tnoremap <C-W><C-W> <C-\><C-n>

" typing q will erase that buffer
augroup Terminal
  autocmd!
  autocmd TermOpen * nnoremap <buffer> q :bd<CR>
augroup END

" Reload when saving vimrc, which is the whole point of changin dotfile
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
command! Soed :n ~/settings/nvim/init.vim<CR>:lcdb<CR>
cab ls :Telescope buffers<CR>
cab gd :terminal export PAGER=don_t_use_me && git diff


" Configure lualine


:set shadafile=~/.nvim.shadafile

:nnoremap <C-P> :Telescope oldfiles<CR>
:nnoremap <C-O> :FF<cr>
:nnoremap <C-I> :Telescope aerial<CR>
set fillchars+=diff:╱


" Setup lua folding
syntax region luaFunction start="function" end="end" fold
syntax region luaBlock start="do" end="end" fold
autocmd FileType lua setlocal foldmethod=syntax



" Common work flow: Switch to current  directory, git write, commit and push.
cab DdG  :lcd %:p:h<CR>:Gwrite<CR>:!git diff --staged %:p <CR> :!read -k <CR>:!git commit %:p <CR>:!git push<CR>
cab DdC :lua GitCommitAndPush()<CR>

:luafile ~/.config/nvim/nvim_init.lua
