
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
Plugin 'sindrets/diffview.nvim'
Plugin 'folke/trouble.nvim'
Plugin 'nvim-telescope/telescope.nvim', { 'tag': '0.1.2' }
Plugin 'stevearc/dressing.nvim'




" Remap terminal keys
:tnoremap <C-W><C-W> <C-\><C-n>

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
:luafile ~/.config/nvim/nvim_init.lua

