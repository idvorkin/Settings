local M = {}

function M.get_current_section()
	local cursor_line = vim.fn.line(".")
	local lines = vim.api.nvim_buf_get_lines(0, 0, cursor_line, false)
	if not lines or #lines == 0 then
		return nil
	end
	for i = #lines, 1, -1 do
		local line = lines[i]
		if line:match("^#+%s+") then
			local section = line:gsub("^#+%s+", ""):gsub("%s+", "-"):lower()
			return section
		end
	end
	return nil
end

function M.parse_youtube_url(url)
	-- Pattern to match a YouTube URL and extract the video ID and timecode
	local pattern = "https://www%.youtube%.com/watch%?v=([%w-_]+)&?t?=?([%d]*)"
	local video_id, timecode = url:match(pattern)

	if video_id then
		local src = video_id
		if timecode then
			src = src .. "&t=" .. timecode -- Use the captured timecode directly
		end
		return string.format('{%% include youtube.html src="%s" %%}', src)
	else
		return nil, "URL does not contain a valid YouTube video ID."
	end
end

function M.get_clipboard_content()
	return vim.fn.getreg("+")
end

function M.print(...)
	print(...)
end

function M.OpenJekyllInBrowser()
	local current_file = vim.fn.expand("%:p")
	local jekyll_root = vim.fn.finddir("_posts", current_file .. ";")
	if jekyll_root ~= "" then
		jekyll_root = vim.fn.fnamemodify(jekyll_root, ":h")
		local relative_path = vim.fn.fnamemodify(current_file, ":~:.")
		-- Read the file content
		local lines = vim.fn.readfile(current_file, "", 10) or {} -- Read first 10 lines, default to empty table if nil
		local permalink = nil
		local in_front_matter = false
		-- Look for permalink in front matter
		for _, line in ipairs(lines) do
			if line == "---" then
				in_front_matter = not in_front_matter
			elseif in_front_matter then
				local key, value = line:match("^(%w+):%s*(.+)$")
				if key == "permalink" then
					permalink = value:gsub("^/", ""):gsub("/$", "")
					break
				end
			end
		end
		local url
		if permalink then
			url = "http://localhost:4000/" .. permalink:gsub("^/", ""):gsub("/$", "")
		else
			-- Check if the file is in a special directory (_ig66 or _d)
			local special_dir = relative_path:match("^_([^/]+)/")
			if special_dir and (special_dir:match("^ig%d+$") or special_dir == "d") then
				url = "http://localhost:4000/"
					.. special_dir
					.. "/"
					.. relative_path:gsub("^_" .. special_dir .. "/", ""):gsub("%.md$", "")
			else
				url = "http://localhost:4000/" .. relative_path:gsub("^_posts/", ""):gsub("%.md$", ".html")
			end
		end

		-- Check if we're in a section and append the anchor
		local current_section = M.get_current_section()
		if current_section then
			url = url .. "#" .. current_section
		end

		if vim.fn.jobstart then
			vim.fn.jobstart({ "open", url })
		else
			M.print("Unable to open browser: vim.fn.jobstart is not available")
		end
	else
		M.print("Not in a Jekyll project")
	end
end

-- Command to open Jekyll in browser
if vim and vim.api and vim.api.nvim_create_user_command then
	vim.api.nvim_create_user_command("OpenJekyllInBrowser", function()
		M.OpenJekyllInBrowser()
	end, {})
end

-- Define a function to get the relative path to the Git repo root
local function get_git_relative_path()
	-- Get the Git repository root directory using a shell command
	local full_path = vim.fn.expand("%:p")
	local git_root = vim.fn.system("git rev-parse --show-toplevel"):gsub("\n", "")

	-- Remove the Git root directory prefix from the full path
	local relative_path = full_path:gsub("^" .. git_root .. "/", "")

	return relative_path
end

-- Define a function to open the GitHub link
local function open_github_link()
	-- Store the current working directory
	local working_dir = vim.fn.getcwd()

	-- Change to the directory of the current file
	local full_path = vim.fn.expand("%:p")
	vim.fn.chdir(vim.fn.fnamemodify(full_path, ":h"))

	-- Get the current line number
	local line_number = vim.fn.line(".")

	-- Get the relative path to the Git repo
	local relative_path = get_git_relative_path()

	-- Get filename onoly
	local path_relative_name = vim.fn.expand("%:t")

	-- Construct the GitHub CLI command
	local command = string.format("gh browse -c %s:%d", path_relative_name, line_number)

	-- Execute the command
	vim.fn.system(command)
	-- Go back to the stored working directory
	vim.fn.chdir(working_dir)
end

-- Create a Vim command to call the Lua function
if vim and vim.api and vim.api.nvim_create_user_command then
	vim.api.nvim_create_user_command("Ghlink", open_github_link, {})
end

return M
