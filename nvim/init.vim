source ~/.vimrc

Plugin 'nvim-lua/plenary.nvim'
Plugin 'nvim-lualine/lualine.nvim'
" If you want to have icons in your statusline choose one of these
Plugin 'nvim-tree/nvim-web-devicons'
Plugin 'Pocco81/true-zen.nvim'
Plugin 'nvim-treesitter/nvim-treesitter', {'do': ':TSUpdate'}

" Reload when saving vimrc, which is the whole point of changin dotfile
augroup reload_luarc | au!
        au BufWritePost *nvim/init.vim source ~/.config/nvim/init.vim
        au BufWritePost *nvim_init.lua source ~/.config/init/nvim.vim
augroup END


" Remap terminal keys
:tnoremap <C-W><C-W> <C-\><C-n>

"
" Reload when saving vimrc, which is the whole point of changin dotfile
augroup reload_nvimrc | au!
        au BufWritePost *init.vim source ~/settings/nvim/init.vim
augroup END

function! RunInteractiveShellCommand(command)
  execute 'terminal ' . a:command
  startinsert
endfunction

command! -nargs=* Shell call RunInteractiveShellCommand(<q-args>)

" Configure lualine


:set shadafile=~/.nvim.shadafile
:luafile ~/.config/nvim/nvim_init.lua

