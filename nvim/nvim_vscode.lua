print("nvim_vscode.lua")

local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"
if not vim.loop.fs_stat(lazypath) then
	vim.fn.system({
		"git",
		"clone",
		"--filter=blob:none",
		"https://github.com/folke/lazy.nvim.git",
		"--branch=stable", -- latest stable release
		lazypath,
	})
end
vim.opt.rtp:prepend(lazypath)
local keymap = vim.api.nvim_set_keymap

function SetupVSCode()
	-- from: https://github.com/milanglacier/nvim/blob/f54b7356dc97cbf9b07a77d5db1ad199c2ff3f2e/lua/conf/vscode.lua#L29
	local function notify(cmd)
		return string.format("<cmd>call VSCodeNotify('%s')<CR>", cmd)
	end

	local function v_notify(cmd)
		return string.format("<cmd>call VSCodeNotifyVisual('%s', 1)<CR>", cmd)
	end

	keymap("n", "<Leader>xr", notify("references-view.findReferences"), { silent = true }) -- language references
	keymap("n", "<Leader>xx", notify("workbench.actions.view.problems"), { silent = true }) -- language diagnostics
	keymap("n", "gr", notify("editor.action.goToReferences"), { silent = true })
	keymap("n", "gd", notify("editor.action.goToDefinition"), { silent = true })
	keymap("n", "<space>rn", notify("editor.action.rename"), { silent = true })
	keymap("n", "<Leader>fm", notify("editor.action.formatDocument"), { silent = true })
	keymap("n", "<Leader>ca", notify("editor.action.refactor"), { silent = true }) -- language code actions

	keymap("n", "<Leader>rg", notify("workbench.action.findInFiles"), { silent = true }) -- use ripgrep to search files
	keymap("n", "<Leader>ts", notify("workbench.action.toggleSidebarVisibility"), { silent = true })
	keymap("n", "<Leader>th", notify("workbench.action.toggleAuxiliaryBar"), { silent = true }) -- toggle docview (help page)
	keymap("n", "<Leader>tp", notify("workbench.action.togglePanel"), { silent = true })
	keymap("n", "<Leader>fc", notify("workbench.action.showCommands"), { silent = true }) -- find commands
	keymap("n", "<Leader>ff", notify("workbench.action.quickOpen"), { silent = true }) -- find files
	keymap("n", "<Leader>tw", notify("workbench.action.terminal.toggleTerminal"), { silent = true }) -- terminal window

	keymap("v", "<Leader>fm", v_notify("editor.action.formatSelection"), { silent = true })
	keymap("v", "<Leader>ca", v_notify("editor.action.refactor"), { silent = true })
	keymap("v", "<Leader>fc", v_notify("workbench.action.showCommands"), { silent = true })
end

SetupVSCode()
