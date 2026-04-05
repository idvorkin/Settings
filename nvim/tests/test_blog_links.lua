local blog_links = require("nvim.nvim_blog_links")
local expect = MiniTest.expect
local new_set = MiniTest.new_set

local T = new_set()

-- parse_link
T["parse_link"] = new_set()

T["parse_link"]["parses markdown link with slug and anchor"] = function()
	local r = blog_links.parse_link("Check out [AI Journal](/ai-journal#diary) for details", 25)
	expect.equality(r, { slug = "/ai-journal", anchor = "diary" })
end

T["parse_link"]["parses markdown link with slug only"] = function()
	local r = blog_links.parse_link("See [AI Journal](/ai-journal) here", 20)
	expect.equality(r, { slug = "/ai-journal" })
end

T["parse_link"]["parses bare slug with anchor"] = function()
	local r = blog_links.parse_link("Visit /ai-journal#diary today", 8)
	expect.equality(r, { slug = "/ai-journal", anchor = "diary" })
end

T["parse_link"]["parses bare slug without anchor"] = function()
	local r = blog_links.parse_link("Visit /ai-journal today", 8)
	expect.equality(r, { slug = "/ai-journal" })
end

T["parse_link"]["returns nil when cursor not on a link"] = function()
	expect.equality(blog_links.parse_link("No links here at all", 5), nil)
end

T["parse_link"]["parses same-page anchor"] = function()
	local r = blog_links.parse_link("Jump to [section](#my-heading) above", 20)
	expect.equality(r, { slug = "", anchor = "my-heading" })
end

T["parse_link"]["handles cursor at start of markdown link"] = function()
	local r = blog_links.parse_link("[text](/foo#bar)", 0)
	expect.equality(r, { slug = "/foo", anchor = "bar" })
end

T["parse_link"]["handles cursor at end of markdown link"] = function()
	local r = blog_links.parse_link("[text](/foo#bar)", 14)
	expect.equality(r, { slug = "/foo", anchor = "bar" })
end

T["parse_link"]["ignores external URLs"] = function()
	expect.equality(blog_links.parse_link("See [Google](https://google.com) here", 18), nil)
end

-- load
T["load"] = new_set({
	hooks = {
		pre_case = function()
			blog_links._clear_cache()
		end,
	},
})

local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"

T["load"]["loads and parses back-links.json from fixture"] = function()
	local data = blog_links.load(fixture_root, "test_back_links.json")
	expect.no_equality(data, nil)
	expect.no_equality(data.redirects, nil)
	expect.no_equality(data.url_info, nil)
	expect.equality(data.redirects["/ai-brain"], "/ai-second-brain")
end

T["load"]["returns nil for missing file"] = function()
	expect.equality(blog_links.load("/nonexistent/path"), nil)
end

T["load"]["caches on repeated calls"] = function()
	local d1 = blog_links.load(fixture_root, "test_back_links.json")
	local d2 = blog_links.load(fixture_root, "test_back_links.json")
	expect.equality(rawequal(d1, d2), true)
end

-- resolve
T["resolve"] = new_set({
	hooks = {
		pre_case = function()
			blog_links._clear_cache()
		end,
	},
})

T["resolve"]["resolves direct slug to absolute path"] = function()
	local path = blog_links.resolve(fixture_root, "/ai-journal", "test_back_links.json")
	expect.equality(path, fixture_root .. "/_d/ai-journal.md")
end

T["resolve"]["follows a single redirect"] = function()
	local path = blog_links.resolve(fixture_root, "/ai-brain", "test_back_links.json")
	expect.equality(path, fixture_root .. "/_d/ai-second-brain.md")
end

T["resolve"]["follows a redirect chain"] = function()
	local path = blog_links.resolve(fixture_root, "/old-page", "test_back_links.json")
	expect.equality(path, fixture_root .. "/_d/final-page.md")
end

T["resolve"]["returns nil for unknown slug"] = function()
	expect.equality(blog_links.resolve(fixture_root, "/nonexistent", "test_back_links.json"), nil)
end

-- browser_url
T["browser_url"] = new_set()

T["browser_url"]["builds URL with slug only"] = function()
	expect.equality(blog_links.browser_url("/ai-journal"), "https://idvork.in/ai-journal")
end

T["browser_url"]["builds URL with slug and anchor"] = function()
	expect.equality(blog_links.browser_url("/ai-journal", "diary"), "https://idvork.in/ai-journal#diary")
end

T["browser_url"]["handles nil anchor"] = function()
	expect.equality(blog_links.browser_url("/ai-journal", nil), "https://idvork.in/ai-journal")
end

return T
