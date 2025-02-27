:echo "Loading VSCode-specific Neovim configuration"

" Source shared configuration
lua nvim_shared.lua

" Define fallbacks for commands/functions that might be missing in VSCode mode
if exists('g:vscode')
  " SoftPencil fallback
  if !exists(':SoftPencil')
    command! -nargs=0 SoftPencil echom "vim-pencil plugin not loaded in VSCode"
  endif
  
  " AutoCorrect fallback
  if !exists('*AutoCorrect')
    function! AutoCorrect()
      echom "vim-autocorrect plugin not loaded in VSCode"
    endfunction
  endif
endif

