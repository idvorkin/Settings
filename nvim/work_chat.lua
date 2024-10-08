print("Hello")
local sqlite = require("sqlite") --- for constructing sql databases
local db = sqlite({
	uri = "~/work.db",
})
db:open()

function GetThreads()
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
      LIMIT 50
    ]]

	local threads = db:eval(query_top_message_per_thread)
	return threads
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

local function chat_pickers(opts)
	opts = opts or {}
	pickers
		.new(opts, {
			prompt_title = "Chats",
			finder = finders.new_table({
				results = GetThreads(),
				entry_maker = function(thread)
					local merged_string = (thread.display_name or "") .. " -- " .. (thread.text or "")
					merged_string = merged_string:gsub("[\n\r]", " "):gsub("%s+", " ")
					return {
						value = merged_string,
						display = merged_string,
						ordinal = merged_string,
					}
				end,
			}),
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

chat_pickers()
-- chat_pickers(require("telescope.themes").get_dropdown{})
