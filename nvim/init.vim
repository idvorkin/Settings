source ~/.vimrc

Plugin 'nvim-lua/plenary.nvim'
Plugin 'nvim-lualine/lualine.nvim'
" If you want to have icons in your statusline choose one of these
Plugin 'nvim-tree/nvim-web-devicons'

" Remap terminal keys

:tnoremap <C-W><C-W> <C-\><C-n>

"
" Reload when saving vimrc, which is the whole point of changin dotfile
augroup reload_nvimrc | au!
        au BufWritePost *init.vim source ~/settings/nvim/init.vim
augroup END
