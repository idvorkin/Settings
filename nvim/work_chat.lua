print("Hello")
local sqlite = require("sqlite") --- for constructing sql databases
local db = sqlite({
	uri = "~/work.db",
})
db:open()

function GetThreadsFromDB()
	local query_top_message_per_thread = [[
    SELECT (coalesce(T.thread_name, STN.thread_name, CPTN.thread_name)) AS display_name ,
        (coalesce(T.thread_name, STN.thread_name, CPTN.thread_name)) AS name ,
        cast( T.thread_key as  text) as uid,
        datetime(T.last_activity_timestamp_ms/1000 + strftime("%s", "1970-01-01") ,"unixepoch","localtime") as lasttime,
        T.snippet as text

      FROM threads AS T
      LEFT OUTER JOIN _cached_participant_thread_info AS CPTN ON CPTN.thread_key = T.thread_key
      LEFT OUTER JOIN _self_thread_name AS STN ON STN.thread_key = T.thread_key
      ORDER BY lasttime DESC
      LIMIT 1000
    ]]

	local threads = db:eval(query_top_message_per_thread)
	return threads
end

function GetThreadMessages(thread_id)
	-- local uid = "26138044329119875"
	local uid = thread_id

	-- Not sure why I can't get string.format to work
	local query_top_message_per_thread = [[
    SELECT
    user.name as user_name,
    datetime(m.timestamp_ms/1000 + strftime("%s", "1970-01-01") ,"unixepoch","localtime") as time,
    m.text
    from messages m
    JOIN user_contact_info AS user ON m.sender_id = user.contact_id
    LEFT OUTER JOIN _cached_participant_thread_info AS CPTN ON CPTN.thread_key = m.thread_key
    LEFT OUTER JOIN _self_thread_name AS STN ON STN.thread_key = m.thread_key
    LEFT OUTER JOIN threads AS T ON T.thread_key = m.thread_key
    where CPTN.thread_name='Igor Dvorkin'
    or T.thread_key=']] .. uid .. [['
    AND m.text <> ''
    order by m.timestamp_ms
    ]]

	return db:eval(query_top_message_per_thread)
end

--local threads_table = {}
--for _, thread in ipairs(threads) do
--local merged_string = (thread.display_name or '') .. ' -- ' .. (thread.text or '')
--merged_string = merged_string:gsub('[\n\r]', ' '):gsub('%s+', ' ')
--table.insert(threads_table, merged_string)
--end
--return threads_table end

-- Make a telescope picker
-- https://github.com/nvim-telescope/telescope.nvim/blob/master/developers.md
--

local pickers = require("telescope.pickers")
local finders = require("telescope.finders")
local conf = require("telescope.config").values

local function threads_finder()
	return finders.new_table({
		results = GetThreadsFromDB(),
		entry_maker = function(thread)
			local merged_string = (thread.display_name or "") .. ":" .. (thread.text or "")
			merged_string = merged_string:gsub("[\n\r]", " "):gsub("%s+", " ")
			return {
				value = thread,
				display = merged_string,
				ordinal = merged_string,
			}
		end,
	})
end
local function thread_preview(opts)
	opts = opts or {}
	local previewers = require("telescope.previewers")
	local thread_viewer = previewers.new_buffer_previewer({
		title = "Thread Preview -- QQ: Why don't i have the entry",
		-- make a table of entry.text repeated 5

		define_preview = function(self, entry, _status)
			local thread = entry.value
			local preview_lines = {}
			--insert every property of thread into preview_lines
			--for k, v in pairs(thread) do
			---- if v is multiline merge it into a single line.
			--local merged_string = (k or "") .. " -- " .. (v or "")
			---- skip if it's empty
			--merged_string = merged_string:gsub("[\n\r]", " "):gsub("%s+", " ")
			--table.insert(preview_lines, merged_string)
			--end
			local messages = GetThreadMessages(thread.uid)
			for _, message in ipairs(messages) do
				local first_name = (message.user_name or ""):match("^(%S+)")
				local merged_string = first_name .. ": " .. (message.text or "")
				-- skip if it's empty
				merged_string = merged_string:gsub("[\n\r]", " "):gsub("%s+", " ")
				table.insert(preview_lines, merged_string)
			end

			for _, e in ipairs(thread) do
				table.insert(preview_lines, e)
			end
			vim.api.nvim_buf_set_lines(self.state.bufnr, 0, -1, false, preview_lines)
			-- Add highlights to preview, might want to do this as a syntax file so it's shared
			-- With the loaded buffer
			-- TODO: Put highlightsin the picker as well.
			local ns_id = vim.api.nvim_create_namespace("highlight_igor")
			local pattern = "Igor" -- Todo load my name from Settings
			for i, line in ipairs(preview_lines) do
				local start, finish = line:find(pattern)
				if start then
					vim.api.nvim_buf_add_highlight(self.state.bufnr, ns_id, "WarningMsg", i - 1, start - 1, finish)
				end
			end
		end,
	})

	return thread_viewer
end

local function chat_pickers(opts)
	opts = opts or {}
	pickers
		.new(opts, {
			prompt_title = "Chats",
			previewer = thread_preview(),
			finder = threads_finder(),
			sorter = conf.generic_sorter(opts),
		})
		:find()
end

--for i, thread in ipairs(GetThreads()) do
---- print(i, thread)
--end

--local colors = { "red", "green", "blue" }
--print (colors)
--for i, thread in ipairs(colors) do
--print(i, thread)
--end

local l = {
	layout_strategy = "vertical",
	layout_config = {
		vertical = {
			prompt_position = "top",
			mirror = true,
			preview_height = 0.7, -- Adjust this value to set the height of the preview window
		},
		preview_cutoff = 1, -- Ensures the preview window is always shown
	},
}
-- chat_pickers(l)
chat_pickers()
-- chat_pickers(require("telescope.themes").get_dropdown{})
