function OpenJekyllInBrowser()
	local permalink = nil
	local keyword = nil

	-- Extract permalink
	for line in io.lines(vim.fn.expand("%")) do
		if line:match("^permalink:") then
			permalink = line:match("^permalink:%s*(%S+)")
			break
		end
	end

	-- Find the nearest section header above the current line
	local current_line_number = vim.fn.line(".")
	for i = current_line_number, 1, -1 do
		local line = vim.fn.getline(i)
		keyword = line:match("^#+%s*(.+)")
		if keyword then
			-- Replace spaces and special characters with hyphens
			keyword = keyword:gsub("%s+", "-"):gsub("[^%w%-]", ""):lower()
		end
		if keyword then
			break
		end
	end

	if permalink then
		local url = "http://localhost:4000" .. permalink
		if keyword then
			url = url .. "#" .. keyword
		end
		os.execute("open " .. url)
	else
		print("No permalink found in the current file.")
	end
end
