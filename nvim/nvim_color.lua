-- Ugh, and down the rabit hole we go!
-- I need more contrast on the dashbar
vim.cmd(":colorscheme catppuccin-macchiato")

options = {
	-- ... the rest of your lualine config
}

-- Create filetype detection for .orgchart files
vim.filetype.add({
	extension = {
		orgchart = "orgchart",
	},
})

vim.api.nvim_create_autocmd({ "FileType" }, {
	pattern = "orgchart",
	callback = function()
		-- Define the syntax matching
		vim.cmd([[
      " Define multi-line comments
      syntax region OrgChartComment start="/\*" end="\*/" contains=@Spell
      highlight link OrgChartComment Comment

      syntax match OrgChartNumbers '\v\|\d+:\d+\|'
      highlight link OrgChartNumbers Title
    ]])
		-- Set foldmethod to indent
		vim.opt_local.foldmethod = "indent"
	end,
})
