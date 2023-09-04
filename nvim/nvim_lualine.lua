local function wordcount()
    return tostring(vim.fn.wordcount().words) .. ' words'
end

local function is_markdown()
    return vim.bo.filetype == "markdown" or vim.bo.filetype == "asciidoc"
end

-- See themes at https://github.com/nvim-lualine/lualine.nvim/blob/master/THEMES.md
require('lualine').setup{
    options= {theme= 'gruvbox_dark'}
}
require("lualine").setup({
  sections = {
      lualine_x = {
          "aerial" ,
          { wordcount,   cond = is_markdown },
      }
  },
})