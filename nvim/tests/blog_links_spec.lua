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
