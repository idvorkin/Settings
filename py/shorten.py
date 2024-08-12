#!python3

from typing_extensions import Optional
import typer
from pathlib import Path
import json
from pydantic import BaseModel, Field, HttpUrl
import httpx
from datetime import datetime
from icecream import ic


app = typer.Typer(help="A Shortening helper")

# Read from secretbox, which is a json file, with key TINURL_TOKEN

token = json.loads( (Path.home()/ "gits/igor2/secretBox.json").read_text())["TINYURL_API_KEY"]



class TinyUrlRequest(BaseModel):
    url: HttpUrl = Field(..., description="The long URL to be shortened")
    domain: Optional[str] = Field(None, description="Optional domain for the shortened URL")
    alias: Optional[str] = Field(None, description="Optional custom alias for the shortened URL")
    tags: Optional[str] = Field(None, description="Optional tags for categorization")
    expires_at: Optional[datetime] = Field(None, description="Optional expiration date and time for the shortened URL")


# generated via gpt.py2json (https://tinyurl.com/23dl535z)
class TinyUrlResponse(BaseModel):
    class Data(BaseModel):
        class Analytics(BaseModel):
            enabled: bool
            public: bool

        alias: str
        analytics: Analytics
        archived: bool
        created_at: str
        deleted: bool
        domain: str
        expires_at: None
        tags: list[str]
        tiny_url: str
        url: str

    code: int
    data: Data
    errors: list[str]

def shorten_url(api_key: str, request_data: TinyUrlRequest) -> TinyUrlResponse:
    url = "https://api.tinyurl.com/create"  # Endpoint for the TinyURL API

    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
    }

    # Convert the request data to a dictionary and ensure all URLs are strings
    request_data_dict = request_data.dict()
    request_data_dict['url'] = str(request_data_dict['url'])

    response = httpx.post(url, headers=headers, json=request_data_dict)

    # Raise an error for bad responses
    response.raise_for_status()

    # Parse the JSON response into a Pydantic model
    ic(response.json())
    return TinyUrlResponse(**response.json())


@app.command()
def shorten(url):
    short = shorten_url(token, TinyUrlRequest(url=url))
    print(short.data.tiny_url)

if __name__ == "__main__":
    app()
