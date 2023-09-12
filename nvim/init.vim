
let g:in_nvim=1
source ~/.vimrc

Plugin 'neovim/nvim-lspconfig'
Plugin 'hrsh7th/cmp-nvim-lsp'
Plugin 'hrsh7th/cmp-buffer'
Plugin 'hrsh7th/cmp-path'
Plugin 'hrsh7th/cmp-cmdline'
Plugin 'hrsh7th/nvim-cmp'
Plugin 'nvim-lua/plenary.nvim'
Plugin 'nvim-lualine/lualine.nvim'
Plugin 'nvim-tree/nvim-web-devicons'
Plugin 'Pocco81/true-zen.nvim'
Plugin 'nvim-treesitter/nvim-treesitter', {'do': ':TSUpdate'}

Plugin 'folke/trouble.nvim'
Plugin 'nvim-telescope/telescope.nvim', { 'tag': '0.1.2' }
Plugin 'stevearc/dressing.nvim'
Plugin 'max397574/better-escape.nvim'
Plugin 'stevearc/aerial.nvim'
Plugin 'zbirenbaum/copilot.lua'
Plugin 'zbirenbaum/copilot-cmp'



" Git like stuff ...
" DiffViewOpen
Plugin 'sindrets/diffview.nvim'
Plugin 'NeogitOrg/neogit'

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
command! Soed :n ~/settings/nvim/init.vim
cab ls :Telescope buffers<CR>
cab gd :terminal git diff


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
