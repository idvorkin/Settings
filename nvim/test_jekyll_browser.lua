local nvim_logic = require("nvim.nvim_logic")
local mock = require("luassert.mock")
local stub = require("luassert.stub")

describe("OpenJekyllInBrowser", function()
    local vim_mock

    before_each(function()
        -- Create a new mock for vim global before each test
        vim_mock = mock({
            fn = {
                expand = function() end,
                finddir = function() end,
                fnamemodify = function() end,
                jobstart = function() end,
                readfile = function() end,
                getreg = function() end  -- Add this line
            },
            api = {
                nvim_create_user_command = function() end
            },
            bo = {
                filetype = ""
            }
        }, true)  -- Use this as a strict mock
        -- Replace the global vim with our mock
        _G.vim = vim_mock
    end)

    after_each(function()
        -- Clear the mock after each test
        mock.clear(vim_mock)
    end)

    it("should open the correct URL for a Jekyll post", function()
        stub(vim_mock.fn, "expand").returns("/path/to/jekyll/_posts/2023-01-01-test-post.md")
        stub(vim_mock.fn, "finddir").returns("/path/to/jekyll/_posts")
        stub(vim_mock.fn, "fnamemodify")
        .on_call_with("/path/to/jekyll/_posts", ":h").returns("/path/to/jekyll")
        .on_call_with("/path/to/jekyll/_posts/2023-01-01-test-post.md", ":~:.").returns("_posts/2023-01-01-test-post.md")

        nvim_logic.OpenJekyllInBrowser()

        assert.stub(vim_mock.fn.jobstart).was_called_with({"open", "http://localhost:4000/2023-01-01-test-post.html"})
    end)

    it("should print an error message when not in a Jekyll project", function()
        stub(vim_mock.fn, "expand").returns("/path/to/non_jekyll/file.md")
        stub(vim_mock.fn, "finddir").returns("")

        local print_spy = spy.on(nvim_logic, "print")
        nvim_logic.OpenJekyllInBrowser()

        assert.spy(print_spy).was_called_with("Not in a Jekyll project")
        assert.stub(vim_mock.fn.jobstart).was_not_called()
        
        -- Add this assertion to ensure the message is exactly as expected
        assert.are.equal("Not in a Jekyll project", print_spy.calls[1].vals[1])
    end)

    it("should handle Jekyll posts with permalinks", function()
        stub(vim_mock.fn, "expand").returns("/path/to/jekyll/_posts/2023-01-01-test-post-with-permalink.md")
        stub(vim_mock.fn, "finddir").returns("/path/to/jekyll/_posts")
        stub(vim_mock.fn, "fnamemodify")
        .on_call_with("/path/to/jekyll/_posts", ":h").returns("/path/to/jekyll")
        .on_call_with("/path/to/jekyll/_posts/2023-01-01-test-post-with-permalink.md", ":~:.").returns("_posts/2023-01-01-test-post-with-permalink.md")

        -- Mock the file content reading to include a permalink
        stub(vim_mock.fn, "readfile").returns({
            "---",
            "layout: post",
            "title: Test Post with Permalink",
            "permalink: /custom-url/",
            "---",
            "Content here..."
        })

        nvim_logic.OpenJekyllInBrowser()

        assert.stub(vim_mock.fn.jobstart).was_called_with({"open", "http://localhost:4000/custom-url"})
    end)

    it("should handle files in special directories without permalinks", function()
        stub(vim_mock.fn, "expand").returns("/path/to/jekyll/_ig66/test-file.md")
        stub(vim_mock.fn, "finddir").returns("/path/to/jekyll/_posts")
        stub(vim_mock.fn, "fnamemodify")
        .on_call_with("/path/to/jekyll/_posts", ":h").returns("/path/to/jekyll")
        .on_call_with("/path/to/jekyll/_ig66/test-file.md", ":~:.").returns("_ig66/test-file.md")

        -- Mock the file content reading to not include a permalink
        stub(vim_mock.fn, "readfile").returns({
            "---",
            "layout: post",
            "title: Test File in Special Directory",
            "---",
            "Content here..."
        })

        nvim_logic.OpenJekyllInBrowser()

        assert.stub(vim_mock.fn.jobstart).was_called_with({"open", "http://localhost:4000/ig66/test-file.html"})
    end)
end)
