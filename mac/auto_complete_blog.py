#!python3
from dataclasses import dataclass
import dataclasses
import json
import os

# See https://www.alfredapp.com/help/workflows/inputs/script-filter/json/
@dataclass
class alfred_item:
    uid: str
    title: str
    subtitle: str
    arg: str
    match: str


class EnhancedJSONEncoder(json.JSONEncoder):
    def default(self, o):
        if dataclasses.is_dataclass(o):
            return dataclasses.asdict(o)
        return super().default(o)


backlinks_path = os.path.expanduser("~/blog/back-links.json")
backlinks = None

with open(backlinks_path) as json_file:
    backlinks = json.load(json_file)

alfred_items = []
redirects = backlinks["redirects"]
urls = backlinks["url_info"]

# Add the pages
for url in urls:
    page_info = urls[url]
    match = f"{page_info['title']} {page_info['url'][1:]}"
    item = alfred_item(
        uid=url, title=page_info["title"], subtitle=url, arg=url, match=match
    )
    alfred_items += [item]

# Add redirects - not sure the best way to handle them are though.
for redirect in redirects:
    alias = redirect[1:]  # strip the /
    actual_path = redirects[redirect]
    actual = redirects[redirect][1:]  # strip the /
    title = f"{urls[actual_path]['title']}" if (actual_path in urls) else actual_path
    match = (
        f"{urls[actual_path]['title']} - {alias}"
        if (actual_path in urls)
        else f"{actual_path} {alias}"
    )
    item = alfred_item(uid=title, title=title, subtitle=alias, arg=actual, match=match)
    alfred_items += [item]


random = alfred_item(uid="random", title="random", subtitle="a random post", arg="random", match="random")
items = {"items": alfred_items+[random]}
print(json.dumps(items, cls=EnhancedJSONEncoder))
