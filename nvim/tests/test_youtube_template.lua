local nvim_logic = require("nvim.nvim_logic")
local expect = MiniTest.expect
local new_set = MiniTest.new_set

local T = new_set()

T["parse_youtube_url"] = new_set()

T["parse_youtube_url"]["returns template for URL with timecode"] = function()
	local template, err = nvim_logic.parse_youtube_url("https://www.youtube.com/watch?v=zjkBMFhNj_g&t=2744")
	expect.equality(template, '{% include youtube.html src="zjkBMFhNj_g&t=2744" %}')
	expect.equality(err, nil)
end

T["parse_youtube_url"]["returns template for URL with timecode in seconds"] = function()
	local template, err = nvim_logic.parse_youtube_url("https://www.youtube.com/watch?v=zjkBMFhNj_g&t=2743s")
	expect.equality(template, '{% include youtube.html src="zjkBMFhNj_g&t=2743" %}')
	expect.equality(err, nil)
end

T["parse_youtube_url"]["returns error for invalid URL"] = function()
	local template, err = nvim_logic.parse_youtube_url("https://www.example.com")
	expect.equality(template, nil)
	expect.equality(err, "URL does not contain a valid YouTube video ID.")
end

return T
