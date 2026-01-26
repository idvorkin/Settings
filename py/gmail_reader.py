#!uv run
# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "typer",
#     "rich",
#     "pydantic",
#     "google-api-python-client",
#     "google-auth-httplib2",
#     "google-auth-oauthlib",
#     "questionary",
#     "requests",
# ]
# ///
"""
Gmail Reader - A command-line application to read and manage Gmail emails.

This application uses the Gmail API to access your Gmail account and provides
various commands to read, search, and manage your emails.
"""

import pickle
import base64
import re
from email.header import decode_header
from pathlib import Path
from datetime import datetime, timedelta
import typer
from rich.console import Console
from rich.table import Table
from rich.progress import Progress
from rich.panel import Panel
from rich.markdown import Markdown
from typing import List, Optional, Any, Dict
from pydantic import BaseModel
import importlib.util
import subprocess
import sys
import time
import shutil
from concurrent.futures import ThreadPoolExecutor, as_completed

try:
    import questionary
except Exception:
    questionary = None
app = typer.Typer(
    help="Gmail Reader - Access and manage your Gmail from the command line",
    no_args_is_help=True,
)
console = Console()


# Check if Gmail API dependencies are available
def check_gmail_dependencies():
    """Check if Gmail API dependencies are installed"""
    dependencies = ["google.auth", "google_auth_oauthlib", "googleapiclient"]

    missing = []
    for dep in dependencies:
        if importlib.util.find_spec(dep) is None:
            missing.append(dep)

    if missing:
        console.print(
            Panel(
                "[bold red]Gmail API dependencies not installed![/bold red]\n\n"
                "Please install the required dependencies:\n"
                "pip install google-api-python-client google-auth-httplib2 google-auth-oauthlib"
            )
        )
        return False
    return True


class EmailMessage(BaseModel):
    """Model for email message data"""

    id: str
    thread_id: str
    subject: str
    sender: str
    date: datetime
    snippet: str
    body_text: Optional[str] = None
    body_html: Optional[str] = None
    labels: List[str] = []

    class Config:
        arbitrary_types_allowed = True


def get_token_path():
    """Get the path to the token file"""
    config_dir = Path.home() / ".config" / "gmail_reader"
    config_dir.mkdir(parents=True, exist_ok=True)
    return config_dir / "token.pickle"


def get_credentials_path():
    """Get the path to the credentials file"""
    config_dir = Path.home() / ".config" / "gmail_reader"
    config_dir.mkdir(parents=True, exist_ok=True)
    return config_dir / "credentials.json"


def authenticate():
    """Authenticate with Gmail API and return the service"""
    # Check if dependencies are installed
    if not check_gmail_dependencies():
        raise typer.Exit(1)

    # Import dependencies here to avoid errors if they're not installed
    from google.auth.transport.requests import Request
    from google_auth_oauthlib.flow import InstalledAppFlow
    from googleapiclient.discovery import build
    from google.auth.exceptions import RefreshError

    # Define the scopes
    SCOPES = ["https://www.googleapis.com/auth/gmail.readonly"]

    creds = None
    token_path = get_token_path()

    # Check if token file exists
    if token_path.exists():
        try:
            with open(token_path, "rb") as token:
                creds = pickle.load(token)

            # Try to refresh token if expired
            if creds and creds.expired and creds.refresh_token:
                try:
                    creds.refresh(Request())
                except RefreshError as e:
                    if "invalid_grant" in str(e):
                        console.print(
                            "[yellow]Token expired. Need to reauthenticate.[/yellow]"
                        )
                        # Delete the invalid token file
                        token_path.unlink()
                        creds = None
                    else:
                        raise
        except Exception as e:
            console.print(
                f"[yellow]Error reading token: {e}. Will create new token.[/yellow]"
            )
            creds = None

    # If no valid credentials available, let the user log in
    if not creds or not creds.valid:
        credentials_path = get_credentials_path()
        if not credentials_path.exists():
            console.print(
                Panel(
                    "[bold red]Credentials file not found![/bold red]\n\n"
                    "You need to create a project in Google Cloud Console and download credentials:\n"
                    "1. Go to [link=https://console.cloud.google.com/]Google Cloud Console[/link]\n"
                    "2. Create a new project\n"
                    "3. Enable the Gmail API\n"
                    "4. Create OAuth 2.0 credentials (Desktop app)\n"
                    "5. Download the credentials.json file\n"
                    f"6. Save it to: [bold]{credentials_path}[/bold]"
                )
            )
            raise typer.Exit(1)

        flow = InstalledAppFlow.from_client_secrets_file(str(credentials_path), SCOPES)
        creds = flow.run_local_server(port=0)

        # Save the credentials for the next run
        with open(token_path, "wb") as token:
            pickle.dump(creds, token)

    # Build the Gmail service
    return build("gmail", "v1", credentials=creds)


def parse_email_date(date_str):
    """Parse email date string to datetime object"""
    # Handle various date formats
    try:
        # Try standard format
        return datetime.strptime(date_str, "%a, %d %b %Y %H:%M:%S %z")
    except ValueError:
        try:
            # Try alternative format
            return datetime.strptime(date_str, "%a, %d %b %Y %H:%M:%S %Z")
        except ValueError:
            # Fallback
            return datetime.now()


def decode_email_subject(subject):
    """Decode email subject with proper encoding"""
    if subject is None:
        return "(No Subject)"

    decoded_parts = []
    for part, encoding in decode_header(subject):
        if isinstance(part, bytes):
            try:
                if encoding:
                    decoded_parts.append(part.decode(encoding))
                else:
                    decoded_parts.append(part.decode("utf-8", errors="replace"))
            except UnicodeDecodeError:
                decoded_parts.append(part.decode("utf-8", errors="replace"))
        else:
            decoded_parts.append(part)

    return "".join(decoded_parts)


def get_email_body(message: Dict[str, Any]) -> tuple:
    """Extract email body (text and HTML) from the message"""
    body_text = None
    body_html = None

    if "payload" not in message:
        return body_text, body_html

    if "parts" in message["payload"]:
        parts = message["payload"]["parts"]
        for part in parts:
            if part["mimeType"] == "text/plain":
                if "data" in part["body"]:
                    body_text = base64.urlsafe_b64decode(part["body"]["data"]).decode(
                        "utf-8", errors="replace"
                    )
            elif part["mimeType"] == "text/html":
                if "data" in part["body"]:
                    body_html = base64.urlsafe_b64decode(part["body"]["data"]).decode(
                        "utf-8", errors="replace"
                    )

            # Check for nested parts
            if "parts" in part:
                for nested_part in part["parts"]:
                    if nested_part["mimeType"] == "text/plain" and not body_text:
                        if "data" in nested_part["body"]:
                            body_text = base64.urlsafe_b64decode(
                                nested_part["body"]["data"]
                            ).decode("utf-8", errors="replace")
                    elif nested_part["mimeType"] == "text/html" and not body_html:
                        if "data" in nested_part["body"]:
                            body_html = base64.urlsafe_b64decode(
                                nested_part["body"]["data"]
                            ).decode("utf-8", errors="replace")
    else:
        # Handle single part messages
        if "body" in message["payload"] and "data" in message["payload"]["body"]:
            data = message["payload"]["body"]["data"]
            if message["payload"]["mimeType"] == "text/plain":
                body_text = base64.urlsafe_b64decode(data).decode(
                    "utf-8", errors="replace"
                )
            elif message["payload"]["mimeType"] == "text/html":
                body_html = base64.urlsafe_b64decode(data).decode(
                    "utf-8", errors="replace"
                )

    return body_text, body_html


def parse_email_message(message: Dict[str, Any]) -> EmailMessage:
    """Parse Gmail API message into EmailMessage model"""
    # Get headers
    headers = {
        header["name"]: header["value"] for header in message["payload"]["headers"]
    }

    # Extract basic info
    subject = decode_email_subject(headers.get("Subject", "(No Subject)"))
    sender = headers.get("From", "Unknown")
    date_str = headers.get("Date", "")
    date = parse_email_date(date_str) if date_str else datetime.now()

    # Get email body
    body_text, body_html = get_email_body(message)

    # Create EmailMessage object
    email_msg = EmailMessage(
        id=message["id"],
        thread_id=message["threadId"],
        subject=subject,
        sender=sender,
        date=date,
        snippet=message.get("snippet", ""),
        body_text=body_text,
        body_html=body_html,
        labels=message.get("labelIds", []),
    )

    return email_msg


def format_date(dt: datetime) -> str:
    """Format date for display"""
    now = datetime.now()
    today = now.date()

    if dt.date() == today:
        return f"Today {dt.strftime('%H:%M')}"
    elif dt.date() == today - timedelta(days=1):
        return f"Yesterday {dt.strftime('%H:%M')}"
    elif (today - dt.date()).days < 7:
        return dt.strftime("%a %H:%M")
    else:
        return dt.strftime("%b %d")


@app.command()
def setup():
    """Set up Gmail API credentials"""
    if not check_gmail_dependencies():
        raise typer.Exit(1)

    credentials_path = get_credentials_path()

    if credentials_path.exists():
        overwrite = typer.confirm("Credentials file already exists. Overwrite?")
        if not overwrite:
            return

    console.print(
        Panel(
            "[bold]Gmail API Credentials Setup[/bold]\n\n"
            "You need to create a project in Google Cloud Console and download credentials:\n"
            "1. Go to [link=https://console.cloud.google.com/]Google Cloud Console[/link]\n"
            "2. Create a new project\n"
            "3. Enable the Gmail API\n"
            "4. Create OAuth 2.0 credentials (Desktop app)\n"
            "5. Download the credentials.json file\n"
            f"6. Save it to: [bold]{credentials_path}[/bold]"
        )
    )

    console.print(
        "\nOnce you've downloaded the credentials file, press Enter to continue..."
    )
    typer.prompt("", default="", show_default=False)

    if credentials_path.exists():
        console.print("[green]Credentials file found![/green]")
        # Test authentication
        try:
            # Authenticate and verify connection by getting user profile
            service = authenticate()
            profile = service.users().getProfile(userId="me").execute()
            email_address = profile.get("emailAddress", "unknown")
            console.print(
                f"[green]Authentication successful! Connected as: {email_address}[/green]"
            )
        except Exception as e:
            console.print(f"[red]Authentication failed: {str(e)}[/red]")
    else:
        console.print(
            "[red]Credentials file not found. Please download and save it to the correct location.[/red]"
        )


@app.command()
def list(
    max_results: int = typer.Option(10, help="Maximum number of emails to retrieve"),
    label: str = typer.Option("INBOX", help="Gmail label to filter by"),
    unread_only: bool = typer.Option(False, help="Show only unread emails"),
    days: int = typer.Option(None, help="Show emails from the last N days"),
):
    """List emails from your Gmail account"""
    try:
        service = authenticate()

        # Build query
        query_parts = []
        if label.upper() != "ALL":
            query_parts.append(f"in:{label}")
        if unread_only:
            query_parts.append("is:unread")
        if days:
            date_limit = (datetime.now() - timedelta(days=days)).strftime("%Y/%m/%d")
            query_parts.append(f"after:{date_limit}")

        query = " ".join(query_parts)

        # Create table
        table = Table(
            title=f"Gmail - {label.upper()}" + (" (Unread)" if unread_only else "")
        )
        table.add_column("Date", style="cyan", no_wrap=True)
        table.add_column("From", style="green")
        table.add_column("Subject", style="bold")
        table.add_column("ID", style="dim")

        with Progress() as progress:
            task = progress.add_task("[cyan]Fetching emails...", total=max_results)

            # Get messages
            results = (
                service.users()
                .messages()
                .list(userId="me", q=query, maxResults=max_results)
                .execute()
            )

            messages = results.get("messages", [])
            progress.update(task, total=len(messages))

            for i, message in enumerate(messages):
                msg = (
                    service.users()
                    .messages()
                    .get(userId="me", id=message["id"])
                    .execute()
                )
                email_msg = parse_email_message(msg)

                # Add to table
                table.add_row(
                    format_date(email_msg.date),
                    email_msg.sender.split("<")[0].strip(),
                    email_msg.subject,
                    email_msg.id,
                )

                progress.update(task, advance=1)

        console.print(table)
        console.print(f"\n[bold]Total:[/bold] {len(messages)} emails")

    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {str(e)}")


def get_kindle_notebook_emails(service, max_results=100, days=None):
    """Get Kindle notebook emails from Gmail

    Args:
        service: Gmail API service instance
        max_results: Maximum number of emails to retrieve
        days: Only retrieve emails from the last N days

    Returns:
        List of EmailMessage objects
    """
    # Build query for Kindle notebook emails
    query_parts = ['from:"Amazon Kindle"', '"You sent a file"']

    if days:
        date_limit = (datetime.now() - timedelta(days=days)).strftime("%Y/%m/%d")
        query_parts.append(f"after:{date_limit}")

    query = " ".join(query_parts)

    with Progress() as progress:
        task = progress.add_task(
            "[cyan]Searching for Kindle notebook emails...", total=max_results
        )

        # Get messages
        results = (
            service.users()
            .messages()
            .list(userId="me", q=query, maxResults=max_results)
            .execute()
        )

        messages = results.get("messages", [])

        if not messages:
            console.print("[yellow]No Kindle notebook emails found.[/yellow]")
            return []

        progress.update(task, total=len(messages))

        # Store message data
        email_data = []

        for i, message in enumerate(messages):
            msg = (
                service.users().messages().get(userId="me", id=message["id"]).execute()
            )
            email_msg = parse_email_message(msg)

            # Add to list
            email_data.append(email_msg)
            progress.update(task, advance=1)

    return email_data


def extract_download_links(html_content):
    """Extract download links from HTML content

    Args:
        html_content: HTML content to search for download links

    Returns:
        List of tuples (url, text) for download links found
    """
    if not html_content:
        return []

    # Look for href attributes with "Download PDF" or similar text
    download_pattern = r'href=[\'"]([^\'"]+)[\'"][^>]*>([^<]*Download[^<]*PDF[^<]*)'
    matches = re.findall(download_pattern, html_content, re.IGNORECASE)

    if matches:
        return matches

    # Try a more general approach to find any links with "download" or "pdf" in them
    download_pattern = r'href=[\'"]([^\'"]+(?:download|pdf)[^\'"]*)[\'"][^>]*>([^<]*)'
    matches = re.findall(download_pattern, html_content, re.IGNORECASE)

    return matches


def copy_to_clipboard(text):
    """Copy text to clipboard

    Args:
        text: Text to copy to clipboard

    Returns:
        True if successful, False otherwise
    """
    try:
        # Different clipboard commands based on OS
        if sys.platform == "darwin":  # macOS
            subprocess.run(["pbcopy"], universal_newlines=True, input=text)
        elif sys.platform == "win32":  # Windows
            subprocess.run(["clip"], universal_newlines=True, input=text)
        else:  # Linux (requires xclip)
            subprocess.run(
                ["xclip", "-selection", "clipboard"],
                universal_newlines=True,
                input=text,
            )

        return True
    except Exception as e:
        console.print(f"[yellow]Could not copy to clipboard: {str(e)}[/yellow]")
        return False


def ping_url(url, attempts=2, delay=1):
    """Ping a URL to warm it up

    Args:
        url: URL to ping
        attempts: Number of ping attempts
        delay: Delay between attempts in seconds

    Returns:
        True if successful, False otherwise
    """
    try:
        import requests

        console.print(
            f"[cyan]Warming up URL with {attempts} pings (delay: {delay}s)...[/cyan]"
        )

        success = False
        for i in range(attempts):
            try:
                response = requests.head(url, timeout=5, allow_redirects=True)
                console.print(
                    f"[cyan]Ping {i + 1}/{attempts}: Status {response.status_code}[/cyan]"
                )
                if response.status_code < 400:
                    success = True
                elif response.status_code == 405:
                    try:
                        response = requests.get(
                            url,
                            timeout=5,
                            allow_redirects=True,
                        )
                        console.print(
                            f"[cyan]Ping {i + 1}/{attempts} (GET): Status {response.status_code}[/cyan]"
                        )
                        if response.status_code < 400:
                            success = True
                    finally:
                        response.close()
            except Exception as e:
                console.print(
                    f"[yellow]Ping {i + 1}/{attempts} failed: {str(e)}[/yellow]"
                )

            if i < attempts - 1:  # Don't sleep after the last attempt
                time.sleep(delay)

        return success
    except Exception as e:
        console.print(f"[yellow]URL warm-up failed: {str(e)}[/yellow]")
        return False


def warm_url_with_retries(
    selected_email,
    selected_url,
    rounds=3,
    attempts=2,
    delay=1,
    round_delay=5,
):
    """Warm a single URL with retries; returns (email, url, ready)."""
    for round_index in range(rounds):
        try:
            if ping_url(selected_url, attempts=attempts, delay=delay):
                return selected_email, selected_url, True
        except Exception as e:
            console.print(
                f"[yellow]Warm-up failed; retrying:[/yellow] {selected_email.subject} ({e})"
            )

        if round_index < rounds - 1:
            console.print(
                f"[cyan]Retrying {selected_email.subject} after {round_delay}s...[/cyan]"
            )
            time.sleep(round_delay)

    return selected_email, selected_url, False


def build_journal_output_path(selected_email):
    """Build a per-email output path based on the subject."""
    match = re.search(r'"([^"]+)"', selected_email.subject)
    if match:
        base_name = match.group(1)
    else:
        base_name = re.sub(r"[^A-Za-z0-9._-]+", "_", selected_email.subject).strip("_")
        if not base_name:
            base_name = selected_email.id[:8]

    base_name = re.sub(r"[^A-Za-z0-9._-]+", "_", base_name).strip("_")
    if not base_name:
        base_name = selected_email.id[:8]

    output_path = Path.cwd() / f"{base_name}-{selected_email.id[:8]}.md"

    cache_dir = Path.home() / "tmp" / "journal"
    cache_path = cache_dir / output_path.name
    return output_path, cache_path


def build_pdf_output_path(
    selected_email: EmailMessage, output_dir: Path | None = None
) -> Path:
    """Build a PDF output path based on the email subject."""
    match = re.search(r'"([^"]+)"', selected_email.subject)
    if match:
        base_name = match.group(1)
    else:
        base_name = re.sub(r"[^A-Za-z0-9._-]+", "_", selected_email.subject).strip("_")
        if not base_name:
            base_name = selected_email.id[:8]

    base_name = re.sub(r"[^A-Za-z0-9._-]+", "_", base_name).strip("_")
    if not base_name:
        base_name = selected_email.id[:8]

    target_dir = output_dir or Path.cwd()
    return target_dir / f"{base_name}.pdf"


def download_pdf(url: str, output_path: Path, warm_first: bool = True) -> bool:
    """Download a PDF from a URL to the specified path.

    Args:
        url: URL to download from
        output_path: Path to save the PDF
        warm_first: Whether to ping the URL first to warm it up

    Returns:
        True if successful, False otherwise
    """
    import requests

    try:
        if warm_first:
            console.print(
                f"[cyan]Warming up download link for {output_path.name}...[/cyan]"
            )
            ping_url(url, attempts=2, delay=1)

        console.print(f"[cyan]Downloading {output_path.name}...[/cyan]")
        response = requests.get(url, timeout=60, allow_redirects=True)
        response.raise_for_status()

        # Check content type
        content_type = response.headers.get("content-type", "")
        if "pdf" not in content_type.lower() and not url.lower().endswith(".pdf"):
            console.print(
                f"[yellow]Warning: {output_path.name} Content-Type is '{content_type}', may not be a PDF[/yellow]"
            )

        # Write the file
        output_path.parent.mkdir(parents=True, exist_ok=True)
        with open(output_path, "wb") as f:
            f.write(response.content)

        file_size = len(response.content)
        console.print(f"[green]Downloaded:[/green] {output_path} ({file_size:,} bytes)")
        return True

    except requests.RequestException as e:
        console.print(f"[red]Download failed for {output_path.name}:[/red] {e}")
        return False
    except IOError as e:
        console.print(f"[red]Failed to save {output_path.name}:[/red] {e}")
        return False


def run_journal_with_retries(
    selected_email,
    selected_url,
    output_path,
    cache_path,
    attempts,
    retry_delay,
    warm_args,
):
    """Run journal with retries; returns True on success."""
    for attempt in range(attempts):
        try:
            console.print(
                f"[cyan]Running journal command with URL...[/cyan] {selected_email.subject}"
            )
            start_time = datetime.now()
            subprocess.run(
                ["journal", "--output", str(output_path), selected_url], check=True
            )
            end_time = datetime.now()
            duration = end_time - start_time
            console.print(
                f"[green]Journal command completed successfully in {duration.total_seconds():.2f} seconds[/green]"
            )
            console.print(f"[green]Processed:[/green] {selected_email.subject}")
            if output_path.exists():
                cache_path.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(output_path, cache_path)
                console.print(f"[green]Cached copy:[/green] {cache_path}")
            return True
        except KeyboardInterrupt:
            # Handle Ctrl+C by copying to clipboard instead
            clipboard_success = copy_to_clipboard(selected_url)
            if clipboard_success:
                console.print(
                    f"[green]Journal canceled. Download link copied to clipboard:[/green] {selected_url}"
                )
            else:
                console.print(
                    f"[yellow]Journal canceled. Here's the URL (manual copy required):[/yellow] {selected_url}"
                )
            raise
        except subprocess.CalledProcessError as e:
            console.print(f"[red]Error running journal command: {e}[/red]")
            if attempt >= attempts - 1:
                return False
            console.print(f"[yellow]Retrying journal after {retry_delay}s...[/yellow]")
            time.sleep(retry_delay)
            _, _, ready = warm_url_with_retries(
                selected_email, selected_url, **warm_args
            )
            if not ready:
                console.print(
                    f"[yellow]Download link not ready; skipping:[/yellow] {selected_email.subject}"
                )
                return False
        except FileNotFoundError:
            console.print("[red]Error: 'journal' command not found[/red]")
            return False

    return False


@app.command()
def kindle(
    max_results: int = typer.Option(
        10, help="Maximum number of Kindle notebook emails to retrieve"
    ),
    days: int = typer.Option(None, help="Show emails from the last N days"),
):
    """Search for Kindle notebook emails (from Amazon Kindle containing 'You sent a file')"""
    try:
        service = authenticate()

        # Get Kindle notebook emails
        emails = get_kindle_notebook_emails(service, max_results, days)

        if not emails:
            return

        # Create table
        table = Table(title="Kindle Notebook Emails")
        table.add_column("Date", style="cyan", no_wrap=True)
        table.add_column("Subject", style="bold")
        table.add_column("ID", style="dim")

        for email in emails:
            table.add_row(format_date(email.date), email.subject, email.id)

        console.print(table)
        console.print(f"\n[bold]Total:[/bold] {len(emails)} Kindle notebook emails")
        console.print(
            "\n[italic]To read a specific email, use:[/italic] gmail read EMAIL_ID"
        )

    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {str(e)}")


@app.command()
def read(
    email_id: str = typer.Argument(..., help="ID of the email to read"),
    format: str = typer.Option(
        "text", help="Display format: text, html, markdown, or dl-link"
    ),
    regular_print: bool = typer.Option(
        False, help="Use regular print instead of rich console (preserves hyperlinks)"
    ),
):
    """Read a specific email by ID"""
    try:
        service = authenticate()

        # Get the email
        msg = service.users().messages().get(userId="me", id=email_id).execute()
        email_msg = parse_email_message(msg)

        # Print email header
        if regular_print:
            print(f"Subject: {email_msg.subject}")
            print(f"From: {email_msg.sender}")
            print(f"Date: {email_msg.date.strftime('%a, %d %b %Y %H:%M:%S')}")
            print("-" * 50)
        else:
            console.print(
                Panel(
                    f"[bold]{email_msg.subject}[/bold]\n"
                    f"[green]From:[/green] {email_msg.sender}\n"
                    f"[cyan]Date:[/cyan] {email_msg.date.strftime('%a, %d %b %Y %H:%M:%S')}\n"
                )
            )

        # Print email body based on format
        if format == "dl-link" and email_msg.body_html:
            # Extract download links from HTML
            matches = extract_download_links(email_msg.body_html)

            if matches:
                if regular_print:
                    print("Download Links Found:")
                    print("-" * 50)
                    for url, text in matches:
                        print(f"Text: {text.strip()}")
                        print(f"URL: {url}")
                        print("-" * 30)
                else:
                    console.print("[bold]Download Links Found:[/bold]")
                    console.print("-" * 50)
                    for url, text in matches:
                        console.print(f"[green]Text:[/green] {text.strip()}")
                        console.print(f"[blue]URL:[/blue] {url}")
                        console.print("-" * 30)
            else:
                if regular_print:
                    print("No download links found in the email.")
                else:
                    console.print(
                        "[yellow]No download links found in the email.[/yellow]"
                    )
        elif format == "html" and email_msg.body_html:
            # Save HTML to a temporary file and open in browser
            temp_file = Path.home() / ".config" / "gmail_reader" / "temp_email.html"
            with open(temp_file, "w") as f:
                f.write(email_msg.body_html)

            if regular_print:
                print("Opening HTML content in your default browser...")
            else:
                console.print("[bold]HTML Content:[/bold]")
                console.print("-" * 50)
                console.print(email_msg.body_html)
                console.print("-" * 50)
                console.print(
                    "[yellow]Opening HTML content in your default browser...[/yellow]"
                )

            import webbrowser

            webbrowser.open(f"file://{temp_file}")
        elif format == "markdown" and (email_msg.body_text or email_msg.body_html):
            # Use body_text if available, otherwise convert HTML to text
            content = email_msg.body_text or email_msg.body_html
            # Handle None case for Markdown
            if content:
                if regular_print:
                    print(content)
                else:
                    console.print(Markdown(content))
            else:
                if regular_print:
                    print("No content available.")
                else:
                    console.print("[yellow]No content available.[/yellow]")
        else:
            # Default to text
            if email_msg.body_text:
                if regular_print:
                    print(email_msg.body_text)
                else:
                    console.print(email_msg.body_text)
            elif email_msg.body_html:
                if regular_print:
                    print("Only HTML content available. Use --format=html to view.")
                    # Show a preview of the HTML content
                    if email_msg.body_html:
                        html_preview = (
                            email_msg.body_html[:500] + "..."
                            if len(email_msg.body_html) > 500
                            else email_msg.body_html
                        )
                        print(html_preview)
                else:
                    console.print(
                        "[yellow]Only HTML content available. Use --format=html to view.[/yellow]"
                    )
                    # Show a preview of the HTML content
                    if email_msg.body_html:
                        html_preview = (
                            email_msg.body_html[:500] + "..."
                            if len(email_msg.body_html) > 500
                            else email_msg.body_html
                        )
                        console.print(html_preview)
            else:
                if regular_print:
                    print("No content available.")
                else:
                    console.print("[yellow]No content available.[/yellow]")

    except Exception as e:
        if regular_print:
            print(f"Error: {str(e)}")
        else:
            console.print(f"[bold red]Error:[/bold red] {str(e)}")


@app.command()
def search(
    query: str = typer.Argument(..., help="Search query"),
    max_results: int = typer.Option(10, help="Maximum number of results"),
):
    """Search for emails using Gmail's search syntax"""
    try:
        service = authenticate()

        # Create table
        table = Table(title=f"Search Results: {query}")
        table.add_column("Date", style="cyan", no_wrap=True)
        table.add_column("From", style="green")
        table.add_column("Subject", style="bold")
        table.add_column("ID", style="dim")

        with Progress() as progress:
            task = progress.add_task("[cyan]Searching...", total=max_results)

            # Get messages
            results = (
                service.users()
                .messages()
                .list(userId="me", q=query, maxResults=max_results)
                .execute()
            )

            messages = results.get("messages", [])

            if not messages:
                console.print("[yellow]No results found.[/yellow]")
                return

            progress.update(task, total=len(messages))

            for message in messages:
                msg = (
                    service.users()
                    .messages()
                    .get(userId="me", id=message["id"])
                    .execute()
                )
                email_msg = parse_email_message(msg)

                # Add to table
                table.add_row(
                    format_date(email_msg.date),
                    email_msg.sender.split("<")[0].strip(),
                    email_msg.subject,
                    email_msg.id,
                )

                progress.update(task, advance=1)

        console.print(table)
        console.print(f"\n[bold]Total:[/bold] {len(messages)} results")

    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {str(e)}")


@app.command()
def labels():
    """List all available Gmail labels"""
    try:
        service = authenticate()

        # Get labels
        results = service.users().labels().list(userId="me").execute()
        labels = results.get("labels", [])

        if not labels:
            console.print("[yellow]No labels found.[/yellow]")
            return

        # Create table
        table = Table(title="Gmail Labels")
        table.add_column("Name", style="green")
        table.add_column("Type", style="cyan")
        table.add_column("ID", style="dim")

        for label in labels:
            table.add_row(label["name"], label["type"], label["id"])

        console.print(table)

    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {str(e)}")


@app.command()
def stats():
    """Show email statistics"""
    try:
        service = authenticate()

        # Get labels for counts
        results = service.users().labels().list(userId="me").execute()
        labels = results.get("labels", [])

        # Create table
        table = Table(title="Gmail Statistics")
        table.add_column("Label", style="green")
        table.add_column("Total", style="cyan", justify="right")
        table.add_column("Unread", style="bold red", justify="right")

        with Progress() as progress:
            task = progress.add_task("[cyan]Fetching statistics...", total=len(labels))

            for label in labels:
                label_info = (
                    service.users().labels().get(userId="me", id=label["id"]).execute()
                )

                # Add to table
                table.add_row(
                    label["name"],
                    str(label_info["messagesTotal"]),
                    str(label_info["messagesUnread"]),
                )

                progress.update(task, advance=1)

        console.print(table)

    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {str(e)}")


@app.command()
def notebook(
    days: int = typer.Option(14, help="Show emails from the last N days"),
    journal: bool = typer.Option(
        True,
        "--journal/--no-journal",
        help="Run 'journal' with the found URL (press Ctrl+C to cancel)",
    ),
    url_only: bool = typer.Option(
        False, help="Just list the URL without copying or journaling"
    ),
    download: bool = typer.Option(
        False, help="Download the PDF file(s) to current directory"
    ),
    output_dir: Optional[str] = typer.Option(
        None, help="Directory to save downloaded PDFs (used with --download)"
    ),
    email_numbers: Optional[str] = typer.Option(
        None, help="Email numbers to select (space or comma separated)"
    ),
):
    """List Kindle notebook emails, let the user select one or more, and journal them (or copy the selected URL(s) to the clipboard with --journal=False)"""
    try:
        service = authenticate()

        # Get Kindle notebook emails
        if days > 14:
            console.print(
                "[yellow]Capping lookback to 14 days (older notebook links won't download).[/yellow]"
            )
            days = 14
        emails = get_kindle_notebook_emails(service, 100, days)

        if not emails:
            return

        # Create table
        table = Table(title="Kindle Notebook Emails")
        table.add_column("#", style="cyan", no_wrap=True)
        table.add_column("Date", style="cyan", no_wrap=True)
        table.add_column("Subject", style="bold")
        table.add_column("ID", style="dim")

        for i, email in enumerate(emails):
            table.add_row(str(i + 1), format_date(email.date), email.subject, email.id)

        console.print(table)
        console.print(f"\n[bold]Total:[/bold] {len(emails)} Kindle notebook emails")

        if email_numbers:
            tokens = re.split(r"[,\s]+", email_numbers.strip())
            try:
                selections = [int(token) for token in tokens if token]
            except ValueError:
                console.print(
                    "[red]Invalid --email-numbers value. Use numbers separated by spaces or commas.[/red]"
                )
                return

            if not selections:
                console.print("[yellow]No emails selected.[/yellow]")
                return

            if any(
                selection < 1 or selection > len(emails) for selection in selections
            ):
                console.print(
                    f"[red]Invalid selection. Enter numbers between 1 and {len(emails)}.[/red]"
                )
                return

            selected_indices = [selection - 1 for selection in selections]
        elif questionary:
            choices = [
                questionary.Choice(
                    f"{i + 1}. {format_date(email.date)} | {email.subject}", value=i
                )
                for i, email in enumerate(emails)
            ]
            selected_indices = questionary.checkbox(
                "Select emails (space to toggle, enter to confirm)", choices=choices
            ).ask()
            if not selected_indices:
                console.print("[yellow]No emails selected.[/yellow]")
                return
        else:
            # Let user select email(s) without questionary installed
            raw_selection = typer.prompt(
                "Enter email number(s) (space or comma separated)", type=str
            )
            tokens = re.split(r"[,\s]+", raw_selection.strip())
            try:
                selections = [int(token) for token in tokens if token]
            except ValueError:
                console.print(
                    "[red]Invalid selection. Use numbers separated by spaces or commas.[/red]"
                )
                return

            if not selections:
                console.print("[yellow]No emails selected.[/yellow]")
                return

            if any(
                selection < 1 or selection > len(emails) for selection in selections
            ):
                console.print(
                    f"[red]Invalid selection. Enter numbers between 1 and {len(emails)}.[/red]"
                )
                return

            selected_indices = [selection - 1 for selection in selections]

        selected_urls = []
        for selection_index in selected_indices:
            selected_email = emails[selection_index]

            if not selected_email.body_html:
                console.print(
                    f"[yellow]No HTML content found in:[/yellow] {selected_email.subject}"
                )
                continue

            # Extract download links from HTML
            matches = extract_download_links(selected_email.body_html)

            if not matches:
                console.print(
                    f"[yellow]No download links found in:[/yellow] {selected_email.subject}"
                )
                continue

            # If multiple links found, let user select which one
            if len(matches) > 1:
                console.print(
                    f"[bold]Multiple download links found for:[/bold] {selected_email.subject}"
                )
                for i, (url, text) in enumerate(matches):
                    console.print(
                        f"{i + 1}. [green]Text:[/green] {text.strip() or 'No text'}"
                    )
                    console.print(f"   [blue]URL:[/blue] {url}")
                    console.print("-" * 30)

                link_selection = typer.prompt(
                    "Enter the number of the link to copy", type=int
                )

                if link_selection < 1 or link_selection > len(matches):
                    console.print(
                        f"[red]Invalid selection. Please enter a number between 1 and {len(matches)}.[/red]"
                    )
                    return

                selected_url = matches[link_selection - 1][0]
            else:
                # Just one link found
                selected_url = matches[0][0]

            selected_urls.append((selected_email, selected_url))

        if not selected_urls:
            console.print(
                "[yellow]No download links found in the selected emails.[/yellow]"
            )
            return

        # Handle the URL(s) based on user preference
        if url_only:
            for selected_email, selected_url in selected_urls:
                console.print(f"[bold]URL:[/bold] {selected_url}")
                console.print(f"[dim]From:[/dim] {selected_email.subject}")
        elif download:
            # Download PDFs directly
            out_dir = Path(output_dir) if output_dir else None
            success_count = 0
            for selected_email, selected_url in selected_urls:
                pdf_path = build_pdf_output_path(selected_email, out_dir)
                console.print(
                    f"\n[bold]Downloading:[/bold] {pdf_path.name} ({selected_email.subject})"
                )
                if download_pdf(selected_url, pdf_path):
                    success_count += 1
            console.print(
                f"\n[green]Downloaded {success_count}/{len(selected_urls)} PDFs[/green]"
            )
        elif journal:
            warm_args = {
                "rounds": 3,
                "attempts": 2,
                "delay": 1,
                "round_delay": 5,
            }
            journal_attempts = 3
            journal_retry_delay = 5
            with ThreadPoolExecutor(max_workers=min(4, len(selected_urls))) as executor:
                future_map = {
                    executor.submit(
                        warm_url_with_retries, selected_email, selected_url, **warm_args
                    ): (selected_email, selected_url)
                    for selected_email, selected_url in selected_urls
                }

                journal_futures = []
                for future in as_completed(future_map):
                    try:
                        selected_email, selected_url, ready = future.result()
                    except Exception as e:
                        selected_email, _ = future_map[future]
                        console.print(
                            f"[yellow]Warm-up failed; skipping:[/yellow] {selected_email.subject} ({e})"
                        )
                        continue
                    if not ready:
                        console.print(
                            f"[yellow]Download link not ready; skipping:[/yellow] {selected_email.subject}"
                        )
                        continue

                    output_path, cache_path = build_journal_output_path(selected_email)
                    journal_futures.append(
                        executor.submit(
                            run_journal_with_retries,
                            selected_email,
                            selected_url,
                            output_path,
                            cache_path,
                            journal_attempts,
                            journal_retry_delay,
                            warm_args,
                        )
                    )

                for future in as_completed(journal_futures):
                    try:
                        future.result()
                    except KeyboardInterrupt:
                        return
        else:
            urls_text = "\n".join(url for _, url in selected_urls)
            clipboard_success = copy_to_clipboard(urls_text)
            if clipboard_success:
                if len(selected_urls) == 1:
                    console.print(
                        f"[green]Download link copied to clipboard:[/green] {selected_urls[0][1]}"
                    )
                else:
                    console.print(
                        f"[green]Download links copied to clipboard:[/green] {len(selected_urls)} links"
                    )
            else:
                if len(selected_urls) == 1:
                    console.print(
                        f"[yellow]Here's the URL (manual copy required):[/yellow] {selected_urls[0][1]}"
                    )
                else:
                    console.print(
                        "[yellow]Here are the URLs (manual copy required):[/yellow]"
                    )
                    for _, selected_url in selected_urls:
                        console.print(selected_url)

    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {str(e)}")


@app.command()
def dexie():
    """Extract the latest Dexie OTP code, copy to clipboard, and show repo info and age"""
    try:
        service = authenticate()

        # Search for Dexie OTP emails (subject: "Your One-Time Password for X")
        query = 'from:dexie.cloud subject:"One-Time Password"'

        with console.status("[cyan]Searching for Dexie OTP emails...[/cyan]"):
            results = (
                service.users()
                .messages()
                .list(userId="me", q=query, maxResults=10)
                .execute()
            )

        messages = results.get("messages", [])

        if not messages:
            console.print("[yellow]No Dexie OTP emails found.[/yellow]")
            console.print(f"[dim]Searched for: {query}[/dim]")
            return

        # Get the most recent email (first in list)
        msg = (
            service.users().messages().get(userId="me", id=messages[0]["id"]).execute()
        )
        email_msg = parse_email_message(msg)

        # Calculate age
        now = (
            datetime.now(email_msg.date.tzinfo)
            if email_msg.date.tzinfo
            else datetime.now()
        )
        age = now - email_msg.date

        # Format age string
        if age.total_seconds() < 60:
            age_str = f"{int(age.total_seconds())} seconds ago"
        elif age.total_seconds() < 3600:
            age_str = f"{int(age.total_seconds() / 60)} minutes ago"
        elif age.total_seconds() < 86400:
            age_str = f"{int(age.total_seconds() / 3600)} hours ago"
        else:
            age_str = f"{int(age.total_seconds() / 86400)} days ago"

        # Get content to search for OTP
        content = email_msg.body_text or email_msg.body_html or email_msg.snippet

        if not content:
            console.print("[red]Could not read email content[/red]")
            return

        # Extract OTP and repo from "Your OTP for [repo]: [code]" format
        # Pattern matches: "Your OTP for humane-tracker.surge.sh: LSR3L9L3"
        # Also handles ports: "Your OTP for 192.168.215.3:3000: C9ZF49VA"
        combined_pattern = r"Your OTP for\s+(.+?):\s+([A-Z0-9]{6,10})\b"

        search_text = email_msg.subject + " " + content
        match = re.search(combined_pattern, search_text, re.IGNORECASE)

        if match:
            repo_name = match.group(1)
            otp_code = match.group(2)
        else:
            # Fallback patterns
            repo_name = "Unknown"
            otp_code = None

            # Try to find alphanumeric OTP code (6-10 chars, uppercase)
            # Require space after colon to avoid matching port numbers like :3000
            otp_match = re.search(r":\s+([A-Z0-9]{6,10})\b", search_text, re.IGNORECASE)
            if otp_match:
                otp_code = otp_match.group(1).upper()

        if not otp_code:
            console.print("[red]Could not extract OTP code from email[/red]")
            console.print(f"[dim]Subject: {email_msg.subject}[/dim]")
            console.print(f"[dim]Snippet: {email_msg.snippet}[/dim]")
            return

        # Copy to clipboard
        clipboard_success = copy_to_clipboard(otp_code)

        # Display results
        console.print()
        console.print(f"[bold green]OTP Code:[/bold green] {otp_code}")
        console.print(f"[bold cyan]Repo:[/bold cyan] {repo_name}")
        console.print(f"[bold yellow]Age:[/bold yellow] {age_str}")
        console.print()

        if clipboard_success:
            console.print("[green]✓ OTP copied to clipboard[/green]")
        else:
            console.print("[yellow]⚠ Could not copy to clipboard[/yellow]")

    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {str(e)}")


@app.command()
def config():
    """Show configuration directories and files used by Gmail Reader"""
    config_dir = Path.home() / ".config" / "gmail_reader"
    token_path = get_token_path()
    credentials_path = get_credentials_path()

    # Create table
    table = Table(title="Gmail Reader Configuration")
    table.add_column("Item", style="cyan")
    table.add_column("Path", style="green")
    table.add_column("Status", style="yellow")

    # Add config directory
    table.add_row(
        "Config Directory",
        str(config_dir),
        "✓ Exists" if config_dir.exists() else "✗ Not Found",
    )

    # Add token file
    table.add_row(
        "Token File",
        str(token_path),
        "✓ Exists" if token_path.exists() else "✗ Not Found",
    )

    # Add credentials file
    table.add_row(
        "Credentials File",
        str(credentials_path),
        "✓ Exists" if credentials_path.exists() else "✗ Not Found",
    )

    console.print(table)

    if not credentials_path.exists():
        console.print(
            "\n[yellow]Note:[/yellow] To set up Gmail Reader, run: [bold]gmail setup[/bold]"
        )


if __name__ == "__main__":
    app()
