local M = {}

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
    if not vim or not vim.fn then
        M.print("vim.fn is not available")
        return
    end

    local current_file = vim.fn.expand("%:p")
    local jekyll_root = vim.fn.finddir("_posts", current_file .. ";")
    if jekyll_root ~= "" then
        jekyll_root = vim.fn.fnamemodify(jekyll_root, ":h")
        local relative_path = vim.fn.fnamemodify(current_file, ":~:.")
        -- Read the file content
        local lines = vim.fn.readfile(current_file, "", 10) or {}  -- Read first 10 lines, default to empty table if nil
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
            url = "http://localhost:4000/" .. relative_path:gsub("^_posts/", ""):gsub("%.md$", ".html")
        end
        vim.fn.jobstart({ "open", url })
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

return M
