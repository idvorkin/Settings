local blog_links = require("nvim.nvim_blog_links")

describe("parse_link", function()
	it("parses markdown link with slug and anchor", function()
		local line = "Check out [AI Journal](/ai-journal#diary) for details"
		local result = blog_links.parse_link(line, 25)
		assert.are.same({ slug = "/ai-journal", anchor = "diary" }, result)
	end)

	it("parses markdown link with slug only", function()
		local line = "See [AI Journal](/ai-journal) here"
		local result = blog_links.parse_link(line, 20)
		assert.are.same({ slug = "/ai-journal" }, result)
	end)

	it("parses bare slug with anchor", function()
		local line = "Visit /ai-journal#diary today"
		local result = blog_links.parse_link(line, 8)
		assert.are.same({ slug = "/ai-journal", anchor = "diary" }, result)
	end)

	it("parses bare slug without anchor", function()
		local line = "Visit /ai-journal today"
		local result = blog_links.parse_link(line, 8)
		assert.are.same({ slug = "/ai-journal" }, result)
	end)

	it("returns nil when cursor not on a link", function()
		local line = "No links here at all"
		local result = blog_links.parse_link(line, 5)
		assert.is_nil(result)
	end)

	it("parses same-page anchor", function()
		local line = "Jump to [section](#my-heading) above"
		local result = blog_links.parse_link(line, 20)
		assert.are.same({ slug = "", anchor = "my-heading" }, result)
	end)

	it("handles cursor at start of markdown link", function()
		local line = "[text](/foo#bar)"
		local result = blog_links.parse_link(line, 0)
		assert.are.same({ slug = "/foo", anchor = "bar" }, result)
	end)

	it("handles cursor at end of markdown link", function()
		local line = "[text](/foo#bar)"
		local result = blog_links.parse_link(line, 14)
		assert.are.same({ slug = "/foo", anchor = "bar" }, result)
	end)

	it("ignores external URLs", function()
		local line = "See [Google](https://google.com) here"
		local result = blog_links.parse_link(line, 18)
		assert.is_nil(result)
	end)
end)

describe("load", function()
	it("loads and parses back-links.json from blog root", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local data = blog_links.load(fixture_root, "test_back_links.json")
		assert.is_not_nil(data)
		assert.is_not_nil(data.redirects)
		assert.is_not_nil(data.url_info)
		assert.equals("/ai-second-brain", data.redirects["/ai-brain"])
	end)

	it("returns nil for missing file", function()
		local data = blog_links.load("/nonexistent/path")
		assert.is_nil(data)
	end)

	it("caches on repeated calls", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local data1 = blog_links.load(fixture_root, "test_back_links.json")
		local data2 = blog_links.load(fixture_root, "test_back_links.json")
		assert.equals(data1, data2)
	end)
end)

describe("resolve", function()
	before_each(function()
		blog_links._clear_cache()
	end)

	it("resolves a direct slug to absolute path", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local path = blog_links.resolve(fixture_root, "/ai-journal", "test_back_links.json")
		assert.equals(fixture_root .. "/_d/ai-journal.md", path)
	end)

	it("follows a single redirect", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local path = blog_links.resolve(fixture_root, "/ai-brain", "test_back_links.json")
		assert.equals(fixture_root .. "/_d/ai-second-brain.md", path)
	end)

	it("follows a redirect chain", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local path = blog_links.resolve(fixture_root, "/old-page", "test_back_links.json")
		assert.equals(fixture_root .. "/_d/final-page.md", path)
	end)

	it("returns nil for unknown slug", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local path = blog_links.resolve(fixture_root, "/nonexistent", "test_back_links.json")
		assert.is_nil(path)
	end)
end)
