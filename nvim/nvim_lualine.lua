local function wordcount()
    return tostring(vim.fn.wordcount().words) .. ' words'
end

local function is_markdown()
    return vim.bo.filetype == "markdown" or vim.bo.filetype == "asciidoc"
end

-- See themes at https://github.com/nvim-lualine/lualine.nvim/blob/master/THEMES.md
require("lualine").setup({
  sections = {
      lualine_z = {
          "aerial" ,
          -- By default, Z is line:column, I don't mind line, but column is too noisy
          { wordcount,   cond = is_markdown },
      }
  },
  options = {
      theme = "catppuccin"
      -- ... the rest of your lualine config
  }
})
