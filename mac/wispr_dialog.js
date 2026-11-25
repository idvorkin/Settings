// The dialog mac/wispr_hands_free.sh opens for dictation: a wide, short bar
// like Alfred's, about three lines of text tall, with one small line under it
// saying what goes with the text and which keys do what. No icon, no header,
// no buttons. JXA rather than AppleScript's `display dialog`, which is small,
// can't be laid out, and is slower to appear.
//
//   osascript -l JavaScript mac/wispr_dialog.js "<what goes with the text>" <click time in ms> [<state file>]
//
// Prints the choice ("Send to Larry" or "Copy to Clipboard"), a newline, then
// the text. Esc prints nothing.
//
//   Return         send to Larry
//   Cmd-Return     copy to the clipboard
//   Option-Return  a new line in the text
//   Esc            cancel
//
// <state file> is the caller's hands-free state. While it says "dialog", Wispr
// is still listening for this dialog: any of the keys above first stops Wispr
// and waits for its text to land here (a few seconds at most), so a cancelled
// dictation is pasted into this dialog and dropped, never into the app behind.
// On stderr, what happened and when, for the caller's log.

ObjC.import("Cocoa");

const SEND = 1000;
const COPY = 1001;
const CANCEL = 1002;
const CHOICES = { [SEND]: "Send to Larry", [COPY]: "Copy to Clipboard" };

const WIDTH_SHARE = 0.5; // of the screen's visible width, like Alfred's bar
const MAX_WIDTH = 1100;
const FONT_SIZE = 22;
const LINES = 3;
const PAD = 14;
const FOOTER = 26; // the small "Sends with" and key-hint line
const WAIT_FOR_PASTE_S = 5; // how long to wait for Wispr's text after stopping it
const SETTLE_S = 0.3; // after the paste lands, before closing

// Globals: the text view's delegate and the menu item's target are weak
// references, and JXA method implementations get no useful `this`.
const state = { file: "", pending: null, hint: null, keys: null };

function log(line) {
	console.log(`dialog: ${line}`);
}

// Wispr is listening for this dialog while the caller's state file says so.
function wisprListening() {
	if (!state.file) return false;
	const body = $.NSString.stringWithContentsOfFileEncodingError(
		state.file,
		$.NSUTF8StringEncoding,
		null,
	);
	return !body.isNil() && body.js.trim() === "dialog";
}

function stopWispr() {
	$.NSFileManager.defaultManager.removeItemAtPathError(state.file, null);
	$.NSTask.launchedTaskWithLaunchPathArguments(
		"/usr/bin/open",
		$(["-g", "wispr-flow://stop-hands-free"]),
	);
}

function closeAfter(seconds) {
	$.NSObject.cancelPreviousPerformRequestsWithTarget(state.keys);
	state.keys.performSelectorWithObjectAfterDelayInModes(
		"doneWaiting:",
		null,
		seconds,
		$([$.NSModalPanelRunLoopMode]),
	);
}

// Every key ends here. Mid-dictation, stop Wispr first and let its paste land.
function finish(code) {
	if (state.pending !== null) return; // already finishing
	if (!wisprListening()) {
		log(`closed: ${CHOICES[code] || "Cancel"}`);
		$.NSApp.stopModalWithCode(code);
		return;
	}
	log(
		`${CHOICES[code] || "Cancel"} while Wispr listens: stopping Wispr, waiting for its text`,
	);
	state.pending = code;
	state.hint.stringValue = "Finishing the dictation…";
	stopWispr();
	closeAfter(WAIT_FOR_PASTE_S);
}

ObjC.registerSubclass({
	name: "WisprDialogKeys",
	superclass: "NSObject",
	methods: {
		// Return sends rather than starting a new line; Option-Return still adds one
		// (insertNewlineIgnoringFieldEditor:). Esc cancels.
		"textView:doCommandBySelector:": {
			types: ["bool", ["id", "SEL"]],
			implementation: (_textView, selector) => {
				if (selector === "insertNewline:") finish(SEND);
				else if (selector === "cancelOperation:") finish(CANCEL);
				else return false;
				return true;
			},
		},
		"textDidChange:": {
			types: ["void", ["id"]],
			implementation: (_notification) => {
				if (state.pending !== null) closeAfter(SETTLE_S); // Wispr's paste landed
			},
		},
		"copyText:": {
			types: ["void", ["id"]],
			implementation: (_sender) => finish(COPY),
		},
		// A menu item whose target is outside the modal window is disabled while
		// the dialog runs modal, unless its target says it works when modal.
		worksWhenModal: {
			types: ["bool", []],
			implementation: () => true,
		},
		"doneWaiting:": {
			types: ["void", ["id"]],
			implementation: (_ignored) => {
				log(
					`closed: ${CHOICES[state.pending] || "Cancel"}, after stopping Wispr`,
				);
				$.NSApp.stopModalWithCode(state.pending);
			},
		},
	},
});

// osascript has no menu bar, and Cmd-V, Cmd-C and the rest only reach the text
// box through an Edit menu's key equivalents. Without this, Wispr's paste (a
// Cmd-V) lands nowhere while typed keys still work. Cmd-Return lives here too.
function addEditMenu(app) {
	const edit = $.NSMenu.alloc.initWithTitle("Edit");
	for (const [title, action, key] of [
		["Undo", "undo:", "z"],
		["Cut", "cut:", "x"],
		["Copy", "copy:", "c"],
		["Paste", "paste:", "v"],
		["Select All", "selectAll:", "a"],
	]) {
		edit.addItemWithTitleActionKeyEquivalent(title, action, key);
	}
	const copyAll = edit.addItemWithTitleActionKeyEquivalent(
		"Copy to Clipboard",
		"copyText:",
		"\r",
	);
	copyAll.target = state.keys;
	const editItem = $.NSMenuItem.alloc.init;
	editItem.submenu = edit;
	const menuBar = $.NSMenu.alloc.init;
	menuBar.addItem(editItem);
	app.mainMenu = menuBar;
}

function label(text, frame, alignment) {
	const field = $.NSTextField.labelWithString(text);
	field.frame = frame;
	field.font = $.NSFont.systemFontOfSize(12);
	field.textColor = $.NSColor.secondaryLabelColor;
	field.alignment = alignment;
	field.lineBreakMode = $.NSLineBreakByTruncatingTail;
	return field;
}

function textBox(frame) {
	const scroll = $.NSScrollView.alloc.initWithFrame(frame);
	scroll.hasVerticalScroller = true;
	scroll.autohidesScrollers = true;
	scroll.borderType = $.NSNoBorder;
	scroll.drawsBackground = false;
	const text = $.NSTextView.alloc.initWithFrame(
		$.NSMakeRect(0, 0, frame.size.width, frame.size.height),
	);
	text.richText = false;
	text.drawsBackground = false;
	text.font = $.NSFont.systemFontOfSize(FONT_SIZE);
	text.textContainerInset = $.NSMakeSize(2, 4);
	text.verticallyResizable = true;
	text.autoresizingMask = $.NSViewWidthSizable;
	text.delegate = state.keys;
	scroll.documentView = text;
	return { scroll, text };
}

function bar(attached) {
	const screen = $.NSScreen.mainScreen.visibleFrame;
	const width = Math.min(
		MAX_WIDTH,
		Math.round(screen.size.width * WIDTH_SHARE),
	);
	const lineHeight = $.NSLayoutManager.alloc.init.defaultLineHeightForFont(
		$.NSFont.systemFontOfSize(FONT_SIZE),
	);
	const textHeight = Math.ceil(lineHeight * LINES) + 8;
	const height = PAD + textHeight + FOOTER;
	// Centred, a third of the way down, where Alfred puts its bar.
	const x = screen.origin.x + (screen.size.width - width) / 2;
	const y = screen.origin.y + screen.size.height * (2 / 3) - height / 2;

	const win = $.NSWindow.alloc.initWithContentRectStyleMaskBackingDefer(
		$.NSMakeRect(x, y, width, height),
		$.NSWindowStyleMaskTitled | $.NSWindowStyleMaskFullSizeContentView,
		$.NSBackingStoreBuffered,
		false,
	);
	win.titlebarAppearsTransparent = true;
	win.titleVisibility = $.NSWindowTitleHidden;
	for (const button of [
		$.NSWindowCloseButton,
		$.NSWindowMiniaturizeButton,
		$.NSWindowZoomButton,
	]) {
		win.standardWindowButton(button).hidden = true;
	}
	win.movableByWindowBackground = true;
	win.level = $.NSFloatingWindowLevel;
	win.releasedWhenClosed = false;

	const box = textBox($.NSMakeRect(PAD, FOOTER, width - 2 * PAD, textHeight));
	const view = win.contentView;
	view.addSubview(box.scroll);
	const split = Math.round(width * 0.5);
	const sendsWith = `Sends with: ${attached || "nothing else"}`;
	view.addSubview(
		label(
			sendsWith,
			$.NSMakeRect(PAD + 2, 6, split - PAD - 12, 16),
			$.NSTextAlignmentLeft,
		),
	);
	const hint = label(
		"↩ Send to Larry    ⌘↩ Copy    ⌥↩ New line    esc Cancel",
		$.NSMakeRect(split, 6, width - split - PAD - 2, 16),
		$.NSTextAlignmentRight,
	);
	view.addSubview(hint);
	return { win, text: box.text, hint };
}

function run(argv) {
	const attached = argv[0] || "";
	const clickedAt = Number(argv[1]) || Date.now();
	state.file = argv[2] || "";
	const app = $.NSApplication.sharedApplication;
	app.setActivationPolicy($.NSApplicationActivationPolicyAccessory);
	state.keys = $.WisprDialogKeys.alloc.init;
	addEditMenu(app);

	const { win, text, hint } = bar(attached);
	state.hint = hint;
	app.activateIgnoringOtherApps(true);
	win.makeKeyAndOrderFront(null);
	win.makeFirstResponder(text);
	log(`shown ${Date.now() - clickedAt} ms after the click`);
	const code = app.runModalForWindow(win);
	win.orderOut(null);
	const choice = CHOICES[code];
	if (choice === undefined) return "";
	return `${choice}\n${text.string.js}`;
}
