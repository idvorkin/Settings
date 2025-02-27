-- Detect Git commit messages
vim.api.nvim_create_autocmd({"BufNewFile", "BufRead"}, {
  pattern = {"COMMIT_EDITMSG", "MERGE_MSG", "TAG_EDITMSG", "NOTES_EDITMSG", "EDIT_DESCRIPTION"},
  callback = function()
    vim.bo.filetype = "gitcommit"
  end
}) 