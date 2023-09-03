
let g:in_nvim=1
source ~/.vimrc

Plugin 'nvim-lua/plenary.nvim'
Plugin 'nvim-lualine/lualine.nvim'
Plugin 'nvim-tree/nvim-web-devicons'
Plugin 'Pocco81/true-zen.nvim'
Plugin 'nvim-treesitter/nvim-treesitter', {'do': ':TSUpdate'}



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

" Configure lualine


:set shadafile=~/.nvim.shadafile
:luafile ~/.config/nvim/nvim_init.lua

