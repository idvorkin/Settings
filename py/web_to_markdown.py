#!python3

import sys
import requests
import html2text
import typer
import re

app = typer.Typer()


def remove_html_tags(text):
    return re.sub("<[^<]+?>", "", text)


@app.command()
def main(
    url: str = typer.Argument(
        ..., help="URL of the webpage to download and convert to markdown."
    ),
    remove_html: bool = typer.Option(
        False, help="Remove any remaining HTML elements in the output markdown."
    ),
    remove_links: bool = typer.Option(False, help="Remove links ."),
):
    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/89.0.4389.82 Safari/537.36",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,image/apng,*/*;q=0.8",
        "Accept-Encoding": "gzip, deflate, br",
        "Accept-Language": "en-US,en;q=0.9",
    }

    response = requests.get(url, headers=headers)

    if response.status_code == 200:
        html_content = response.text
        text_maker = html2text.HTML2Text()
        text_maker.ignore_links = True
        text_maker.body_width = 0
        markdown_output = text_maker.handle(html_content)

        if remove_html:
            markdown_output = remove_html_tags(markdown_output)

        sys.stdout.write(markdown_output)
    else:
        typer.echo(
            f"Error: Unable to download the webpage. HTTP status code: {response.status_code}",
            err=True,
        )


if __name__ == "__main__":
    app()
