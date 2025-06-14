#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "typer",
#     "rich",
#     "pyobjc-framework-Cocoa",
# ]
# ///

import typer
from typing_extensions import Annotated
from rich.console import Console
from rich.panel import Panel
import time

# PyObjC imports
import AppKit
import Foundation

app = typer.Typer(
    help="Show floating windows with messages",
    no_args_is_help=True,
)

console = Console()


def create_floating_window(text, duration, position="center"):
    """Create a floating window with the given text"""
    # Get main screen dimensions
    screen = AppKit.NSScreen.mainScreen()
    screen_frame = screen.frame()

    # Create a temporary text field to measure text size
    font = AppKit.NSFont.boldSystemFontOfSize_(18)
    temp_text_field = AppKit.NSTextField.alloc().init()
    temp_text_field.setFont_(font)
    temp_text_field.setStringValue_(text)

    # Calculate text size
    text_size = temp_text_field.cell().cellSizeForBounds_(
        Foundation.NSMakeRect(0, 0, 1000, 1000)
    )

    # Add padding around the text
    padding_x = 40  # 20px on each side
    padding_y = 40  # 20px on top and bottom

    # Calculate window dimensions based on text size
    width = max(200, text_size.width + padding_x)  # Minimum width of 200
    height = max(80, text_size.height + padding_y)  # Minimum height of 80

    # Position the window based on the position parameter
    if position == "corner":
        # Top-right corner with some margin
        margin = 20
        x = screen_frame.size.width - width - margin
        y = screen_frame.size.height - height - margin
    else:  # center
        # Center the window on screen
        x = (screen_frame.size.width - width) / 2
        y = (screen_frame.size.height - height) / 2 + 100  # Slightly above center

    window_rect = Foundation.NSMakeRect(x, y, width, height)

    # Window style - borderless for clean look
    style_mask = AppKit.NSWindowStyleMaskBorderless

    # Create the window directly
    window = AppKit.NSWindow.alloc().initWithContentRect_styleMask_backing_defer_(
        window_rect, style_mask, AppKit.NSBackingStoreBuffered, False
    )

    # Configure window properties
    window.setLevel_(AppKit.NSFloatingWindowLevel + 1)
    window.setOpaque_(False)
    window.setBackgroundColor_(
        AppKit.NSColor.colorWithCalibratedRed_green_blue_alpha_(0.1, 0.1, 0.1, 0.9)
    )
    window.setAlphaValue_(0.95)
    window.setReleasedWhenClosed_(True)

    # Create content view
    content_view = AppKit.NSView.alloc().initWithFrame_(
        Foundation.NSMakeRect(0, 0, width, height)
    )

    # Create text field that fills the content area with padding
    text_field = AppKit.NSTextField.alloc().initWithFrame_(
        Foundation.NSMakeRect(
            padding_x / 2, padding_y / 2, width - padding_x, height - padding_y
        )
    )
    text_field.setStringValue_(text)
    text_field.setBezeled_(False)
    text_field.setDrawsBackground_(False)
    text_field.setEditable_(False)
    text_field.setSelectable_(False)
    text_field.setAlignment_(AppKit.NSTextAlignmentCenter)
    text_field.setFont_(font)
    text_field.setTextColor_(AppKit.NSColor.whiteColor())

    # Enable word wrapping for long text
    text_field.cell().setWraps_(True)
    text_field.cell().setScrollable_(False)

    # Add text field to content view
    content_view.addSubview_(text_field)
    window.setContentView_(content_view)

    return window


def show_window(message, seconds, position):
    """Common function to show a window"""
    console.print(
        Panel(
            f"Showing floating window ({position}): [bold]{message}[/] for {seconds} seconds",
            title="ux.py",
        )
    )

    # Ensure we have an NSApplication instance
    app_instance = AppKit.NSApplication.sharedApplication()
    app_instance.setActivationPolicy_(AppKit.NSApplicationActivationPolicyAccessory)

    # Create and show the floating window
    window = create_floating_window(message, seconds, position)
    window.makeKeyAndOrderFront_(None)
    window.orderFrontRegardless()

    # Keep the application running for the specified duration
    # Run the event loop to handle window events
    end_time = time.time() + seconds + 0.5  # Add small buffer
    while time.time() < end_time:
        # Process events
        event = app_instance.nextEventMatchingMask_untilDate_inMode_dequeue_(
            AppKit.NSAnyEventMask,
            Foundation.NSDate.dateWithTimeIntervalSinceNow_(0.1),
            Foundation.NSDefaultRunLoopMode,
            True,
        )
        if event:
            app_instance.sendEvent_(event)

        # Small sleep to prevent high CPU usage
        time.sleep(0.05)

    # Close the window
    window.close()
    console.print("[green]Window closed[/green]")


@app.command()
def center(
    message: Annotated[
        str, typer.Argument(help="Message to display in the floating window")
    ],
    seconds: Annotated[
        int, typer.Option("--seconds", help="How long to show the window")
    ] = 5,
):
    """
    Show a floating window with the given message in the center of the screen.
    """
    show_window(message, seconds, "center")


@app.command()
def corner(
    message: Annotated[
        str, typer.Argument(help="Message to display in the floating window")
    ],
    seconds: Annotated[
        int, typer.Option("--seconds", help="How long to show the window")
    ] = 5,
):
    """
    Show a floating window with the given message in the top-right corner.
    """
    show_window(message, seconds, "corner")


if __name__ == "__main__":
    app()
