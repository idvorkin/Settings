-- Load the nvim_logic file to access the parse_youtube_url function
local nvim_logic = require("nvim.nvim_logic")
require("plenary.test_harness")
local plenary_ok = pcall(require, "plenary")
assert(False)

describe("parse_youtube_url", function()
	it("should return the correct template string for a YouTube URL with a timecode", function()
		local url = "https://www.youtube.com/watch?v=zjkBMFhNj_g&t=2744"
		local template, err = nvim_logic.parse_youtube_url(url)
		print("    Actual template:", template)
		print("    Error:", err)
		assert(
			template == '{% include youtube.html src="zjkBMFhNj_g&t=2744" %}',
			'Expected template string not returned.\nExpected:\n{% include youtube.html src="zjkBMFhNj_g&t=2744" %}\nGot:\n'
				.. tostring(template)
		)
		assert(err == nil, "Expected error to be nil, but got: " .. tostring(err))
	end)

	it("should return the correct template string for a YouTube URL with a timecode in seconds", function()
		local url = "https://www.youtube.com/watch?v=zjkBMFhNj_g&t=2743s"
		local template, err = nvim_logic.parse_youtube_url(url)
		print("    Actual template:", template)
		print("    Error:", err)
		assert(
			template == '{% include youtube.html src="zjkBMFhNj_g&t=2743" %}',
			'Expected template string not returned.\nExpected:\n{% include youtube.html src="zjkBMFhNj_g&t=2743" %}\nGot:\n'
				.. tostring(template)
		)
		assert(err == nil, "Expected error to be nil, but got: " .. tostring(err))
	end)

	it("should return an error for an invalid YouTube URL", function()
		local url = "https://www.example.com"
		local template, err = nvim_logic.parse_youtube_url(url)
		assert(template == nil, "Expected template to be nil, but got: " .. tostring(template))
		assert(
			err == "URL does not contain a valid YouTube video ID.",
			"Expected error message doesn't match. Got: " .. tostring(err)
		)
	end)
end)
