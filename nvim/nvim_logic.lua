local function parse_youtube_url(url)
  -- Pattern to match a YouTube URL and extract the video ID and timecode
  local pattern = 'https://www%.youtube%.com/watch%?v=([%w-_]+)&?t?=?([%d]*)'
  local video_id, timecode = url:match(pattern)

  if video_id then
    local src = video_id
    if timecode then
      src = src .. "&t=" .. timecode  -- Use the captured timecode directly
    end
    return string.format('{%% include youtube.html src="%s" %%}', src)
  else
    return nil, 'URL does not contain a valid YouTube video ID.'
  end
end

local function get_clipboard_content()
  return vim.fn.getreg('+')
end

return {
  parse_youtube_url = parse_youtube_url,
  get_clipboard_content = get_clipboard_content
}
