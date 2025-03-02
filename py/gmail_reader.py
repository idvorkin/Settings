#!/usr/bin/env python3
"""
Gmail Reader - A command-line application to read and manage Gmail emails.

This application uses the Gmail API to access your Gmail account and provides
various commands to read, search, and manage your emails.
"""

import pickle
import base64
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

    # Define the scopes
    SCOPES = ["https://www.googleapis.com/auth/gmail.readonly"]

    creds = None
    token_path = get_token_path()

    # Check if token file exists
    if token_path.exists():
        with open(token_path, "rb") as token:
            creds = pickle.load(token)

    # If credentials don't exist or are invalid, get new ones
    if not creds or not creds.valid:
        if creds and creds.expired and creds.refresh_token:
            creds.refresh(Request())
        else:
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

            flow = InstalledAppFlow.from_client_secrets_file(
                str(credentials_path), SCOPES
            )
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
def list_emails(
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


@app.command()
def read_email(
    email_id: str = typer.Argument(..., help="ID of the email to read"),
    format: str = typer.Option("text", help="Display format: text, html, or markdown"),
):
    """Read a specific email by ID"""
    try:
        service = authenticate()

        # Get the email
        msg = service.users().messages().get(userId="me", id=email_id).execute()
        email_msg = parse_email_message(msg)

        # Print email header
        console.print(
            Panel(
                f"[bold]{email_msg.subject}[/bold]\n"
                f"[green]From:[/green] {email_msg.sender}\n"
                f"[cyan]Date:[/cyan] {email_msg.date.strftime('%a, %d %b %Y %H:%M:%S')}\n"
            )
        )

        # Print email body based on format
        if format == "html" and email_msg.body_html:
            # Save HTML to a temporary file and open in browser
            temp_file = Path.home() / ".config" / "gmail_reader" / "temp_email.html"
            with open(temp_file, "w") as f:
                f.write(email_msg.body_html)

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
                console.print(Markdown(content))
            else:
                console.print("[yellow]No content available.[/yellow]")
        else:
            # Default to text
            if email_msg.body_text:
                console.print(email_msg.body_text)
            elif email_msg.body_html:
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
                console.print("[yellow]No content available.[/yellow]")

    except Exception as e:
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
def kindle_notebook(
    max_results: int = typer.Option(
        10, help="Maximum number of Kindle notebook emails to retrieve"
    ),
    days: int = typer.Option(None, help="Show emails from the last N days"),
):
    """Search for Kindle notebook emails (from Amazon Kindle containing 'You sent a file')"""
    try:
        service = authenticate()

        # Build query for Kindle notebook emails
        query_parts = ['from:"Amazon Kindle"', '"You sent a file"']

        if days:
            date_limit = (datetime.now() - timedelta(days=days)).strftime("%Y/%m/%d")
            query_parts.append(f"after:{date_limit}")

        query = " ".join(query_parts)

        # Create table
        table = Table(title="Kindle Notebook Emails")
        table.add_column("Date", style="cyan", no_wrap=True)
        table.add_column("Subject", style="bold")
        table.add_column("ID", style="dim")

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
                    format_date(email_msg.date), email_msg.subject, email_msg.id
                )

                progress.update(task, advance=1)

        console.print(table)
        console.print(f"\n[bold]Total:[/bold] {len(messages)} Kindle notebook emails")
        console.print(
            "\n[italic]To read a specific email, use:[/italic] gmail read-email EMAIL_ID"
        )

    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {str(e)}")


if __name__ == "__main__":
    app()
