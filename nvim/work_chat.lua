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
    order by m.timestamp_ms DESC
    LIMIT 20
    ]]

	return db:eval(query_top_message_per_thread)
end

-- TODO: Remove Boring Messages (bots, notifs, etc)

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

-- Define highlight groups (you might want to put this in your init.lua or a separate file)
local function setup_highlight_groups()
	local groups = {
		ChatName1 = { fg = "#90EE90" },  -- Light Green
		ChatName2 = { fg = "#ADD8E6" },  -- Light Blue
		ChatName3 = { fg = "#FFA07A" },  -- Light Salmon
		ChatName4 = { fg = "#DDA0DD" },  -- Plum
		ChatName5 = { fg = "#FFB6C1" },  -- Light Pink
		ChatNameSelf = { fg = "#FFFF00", bold = true },  -- Yellow (bold)
		-- Add more as needed
	}
	
	for group_name, attributes in pairs(groups) do
		vim.api.nvim_set_hl(0, group_name, attributes)
	end
end

-- Call this function when your plugin loads
setup_highlight_groups()

local self = "Igor"

local function apply_highlights(bufnr, lines)
	local ns_id = vim.api.nvim_create_namespace("chat_highlights")
	local name_groups = {}
	local group_index = 1
	local max_groups = 5  -- Matches the number of ChatName groups we defined

	for i, line in ipairs(lines) do
		local name = line:match("^(%S+):")
		if name then
			if name == self then
				name_groups[name] = "ChatNameSelf"
			elseif not name_groups[name] then
				name_groups[name] = "ChatName" .. group_index
				group_index = (group_index % max_groups) + 1
			end
			local start, finish = line:find(name)
			if start then
				vim.api.nvim_buf_add_highlight(bufnr, ns_id, name_groups[name], i - 1, start - 1, finish)
			end
		end
	end
end

local function thread_preview(opts)
	opts = opts or {}
	local previewers = require("telescope.previewers")
	local thread_viewer = previewers.new_buffer_previewer({
		title = "Thread Preview -- QQ: Why don't i have the entry",
		define_preview = function(self, entry, _status)
			local thread = entry.value
			local preview_lines = {}
			local messages = GetThreadMessages(thread.uid)
			for _, message in ipairs(messages) do
				local first_name = (message.user_name or ""):match("^(%S+)")
				local merged_string = first_name .. ": " .. (message.text or "")
				merged_string = merged_string:gsub("[\n\r]", " "):gsub("%s+", " ")
				table.insert(preview_lines, merged_string)
			end

			for _, e in ipairs(thread) do
				table.insert(preview_lines, e)
			end
			vim.api.nvim_buf_set_lines(self.state.bufnr, 0, -1, false, preview_lines)
			
			apply_highlights(self.state.bufnr, preview_lines)
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
			attach_mappings = function(prompt_bufnr, map)
				local actions = require("telescope.actions")
				local action_state = require("telescope.actions.state")

				actions.select_default:replace(function()
					actions.close(prompt_bufnr)
					local selection = action_state.get_selected_entry()
					local thread = selection.value

					-- Create a new buffer
					local buf = vim.api.nvim_create_buf(true, true)
					vim.api.nvim_buf_set_option(buf, 'buftype', 'nofile')
					vim.api.nvim_buf_set_option(buf, 'bufhidden', 'wipe')
					vim.api.nvim_buf_set_name(buf, "Thread: " .. thread.display_name)

					-- Get messages for the selected thread
					local messages = GetThreadMessages(thread.uid)
					local buf_lines = {}
					for _, message in ipairs(messages) do
						local first_name = (message.user_name or ""):match("^(%S+)")
						local merged_string = first_name .. ": " .. (message.text or "")
						merged_string = merged_string:gsub("[\n\r]", " "):gsub("%s+", " ")
						table.insert(buf_lines, merged_string)
					end

					-- Set the buffer contents
					vim.api.nvim_buf_set_lines(buf, 0, -1, false, buf_lines)

					-- Open the buffer in a new window
					vim.api.nvim_command('vsplit')
					vim.api.nvim_win_set_buf(0, buf)

					apply_highlights(buf, buf_lines)
				end)

				return true
			end,
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
			preview_height = 0.5, -- Adjust this value to set the height of the preview window
		},
		preview_cutoff = 1, -- Ensures the preview window is always shown
	},
}
chat_pickers(l)
-- chat_pickers()
-- chat_pickers(require("telescope.themes").get_dropdown{})
