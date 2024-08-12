#!python3

from typing_extensions import Optional
import typer
from pathlib import Path
import json
from pydantic import BaseModel, Field, HttpUrl
import httpx
from datetime import datetime


app = typer.Typer(help="A Shortening helper")

# Read from secretbox, which is a json file, with key TINURL_TOKEN

token = json.loads( (Path.home()/ "gits/igor2/secretBox.json").read_text())["TINYURL_API_KEY"]



class TinyUrlRequest(BaseModel):
    url: HttpUrl = Field(..., description="The long URL to be shortened")
    domain: Optional[str] = Field(None, description="Optional domain for the shortened URL")
    alias: Optional[str] = Field(None, description="Optional custom alias for the shortened URL")
    tags: Optional[str] = Field(None, description="Optional tags for categorization")
    expires_at: Optional[datetime] = Field(None, description="Optional expiration date and time for the shortened URL")


class TinyUrlResponse(BaseModel):
    tiny_url: HttpUrl = Field(..., description="The shortened URL")
    alias: Optional[str] = Field(None, description="The alias used for the shortened URL")
    long_url: HttpUrl = Field(..., description="The original long URL")
    created_at: datetime = Field(..., description="The date and time when the URL was shortened")
    expires_at: Optional[datetime] = Field(None, description="The expiration date and time for the shortened URL, if applicable")

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
    return TinyUrlResponse(**response.json())


@app.command()
def shorten(url):
    short = shorten_url(token, TinyUrlRequest(url=url))
    ic(short)

if __name__ == "__main__":
    app()
