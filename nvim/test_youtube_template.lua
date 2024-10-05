-- Load the nvim_logic file to access the parse_youtube_url function
local nvim_logic = require("nvim.nvim_logic")

local total_tests = 0
local passed_tests = 0

-- Define simple test functions
local function describe(desc, func)
    print("\nDescribing: " .. desc)
    func()
end

local function it(desc, func)
    total_tests = total_tests + 1
    print("\n  It " .. desc)
    local status, error = pcall(func)
    if status then
        print("    ✓ PASS")
        passed_tests = passed_tests + 1
    else
        print("    ✗ FAIL: " .. tostring(error))
    end
end

describe("parse_youtube_url", function()
    it("should return the correct template string for a YouTube URL with a timecode", function()
        local url = "https://www.youtube.com/watch?v=zjkBMFhNj_g&t=2744"
        local template, err = nvim_logic.parse_youtube_url(url)
        print("    Actual template:", template)
        print("    Error:", err)
        assert(template == '{% include youtube.html src="zjkBMFhNj_g&t=2744" %}',
            'Expected template string not returned.\nExpected:\n{% include youtube.html src="zjkBMFhNj_g&t=2744" %}\nGot:\n'
                .. tostring(template))
        assert(err == nil, "Expected error to be nil, but got: " .. tostring(err))
    end)

    it("should return the correct template string for a YouTube URL with a timecode in seconds", function()
        local url = "https://www.youtube.com/watch?v=zjkBMFhNj_g&t=2743s"
        local template, err = nvim_logic.parse_youtube_url(url)
        print("    Actual template:", template)
        print("    Error:", err)
        assert(template == '{% include youtube.html src="zjkBMFhNj_g&t=2743" %}',
            'Expected template string not returned.\nExpected:\n{% include youtube.html src="zjkBMFhNj_g&t=2743" %}\nGot:\n'
                .. tostring(template))
        assert(err == nil, "Expected error to be nil, but got: " .. tostring(err))
    end)

    it("should return an error for an invalid YouTube URL", function()
        local url = "https://www.example.com"
        local template, err = nvim_logic.parse_youtube_url(url)
        assert(template == nil, "Expected template to be nil, but got: " .. tostring(template))
        assert(err == "URL does not contain a valid YouTube video ID.", "Expected error message doesn't match. Got: " .. tostring(err))
    end)
end)

print("\n--- Test Summary ---")
print(string.format("Passed: %d/%d", passed_tests, total_tests))
print(string.format("Failed: %d/%d", total_tests - passed_tests, total_tests))
print("-------------------")

if passed_tests == total_tests then
    os.exit(0)  -- All tests passed
else
    os.exit(1)  -- Some tests failed
end
