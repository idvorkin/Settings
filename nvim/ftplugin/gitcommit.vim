" Custom folding for Git commit messages
" This file is loaded when editing Git commit messages

" Set folding method to syntax
setlocal foldmethod=expr
setlocal foldexpr=GitCommitFoldExpr(v:lnum)
setlocal foldtext=GitCommitFoldText()

" Don't fold by default
setlocal nofoldenable

" Define custom fold expression for Git commit messages
function! GitCommitFoldExpr(lnum)
  let line = getline(a:lnum)
  
  " Fold comment lines (lines starting with #)
  if line =~ '^#'
    " Check if this is a section header
    if line =~ '^# \(Changes\|Untracked\|Your branch\|On branch\|Not currently\|Unmerged\)'
      return '>1'
    elseif getline(a:lnum - 1) !~ '^#'
      " Start a new fold if previous line is not a comment
      return '>1'
    else
      " Continue the fold
      return '1'
    endif
  endif
  
  " Don't fold non-comment lines (the actual commit message)
  return '0'
endfunction

" Define custom fold text for Git commit messages
function! GitCommitFoldText()
  let line = getline(v:foldstart)
  
  " Extract the section name for comment sections
  if line =~ '^# \(Changes\|Untracked\|Your branch\|On branch\|Not currently\|Unmerged\)'
    let section = matchstr(line, '^# \zs.*')
    return '▶ ' . section . ' (' . (v:foldend - v:foldstart + 1) . ' lines)'
  else
    " For other comment sections
    return '▶ Git comments (' . (v:foldend - v:foldstart + 1) . ' lines)'
  endif
endfunction

" Add key mappings for folding
nnoremap <buffer> <Tab> za
nnoremap <buffer> <S-Tab> zM

" Automatically fold comments when opening the commit message
augroup GitCommitFolding
  autocmd!
  autocmd BufWinEnter COMMIT_EDITMSG setlocal foldmethod=expr
  autocmd BufWinEnter COMMIT_EDITMSG setlocal foldexpr=GitCommitFoldExpr(v:lnum)
  autocmd BufWinEnter COMMIT_EDITMSG setlocal foldtext=GitCommitFoldText()
  " Fold all comment sections by default
  autocmd BufWinEnter COMMIT_EDITMSG normal zM
  " But unfold the first section (usually "Changes to be committed")
  autocmd BufWinEnter COMMIT_EDITMSG normal gg
  autocmd BufWinEnter COMMIT_EDITMSG normal zo
augroup END

" Set other useful options for commit messages
setlocal spell
setlocal textwidth=72
setlocal colorcolumn=+1 