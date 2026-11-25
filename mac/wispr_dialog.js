// The dialog mac/wispr_hands_free.sh opens for dictation: a large text box,
// sized as a share of the screen, with Send to Larry, Copy to Clipboard and
// Cancel. JXA rather than AppleScript's `display dialog`, which is small, can't
// be resized, and is slower to appear.
//
//   osascript -l JavaScript mac/wispr_dialog.js "<what goes with the text>" <click time in ms>
//
// Prints the button, a newline, then the text. Cancel or Esc prints nothing.
// Return sends; Option-Return adds a line to the text. On stderr, how
// long after the click the dialog showed, for the caller's log.

ObjC.import("Cocoa");

const WIDTH_SHARE = 0.5; // of the screen's visible width
const HEIGHT_SHARE = 0.4; // of the screen's visible height
const BUTTONS = ["Send to Larry", "Copy to Clipboard", "Cancel"];

function textBox(width, height) {
	const scroll = $.NSScrollView.alloc.initWithFrame($.NSMakeRect(0, 0, width, height));
	scroll.hasVerticalScroller = true;
	scroll.borderType = $.NSBezelBorder;
	const text = $.NSTextView.alloc.initWithFrame($.NSMakeRect(0, 0, width, height));
	text.richText = false;
	text.font = $.NSFont.systemFontOfSize(28); // about Alfred's search text
	text.textContainerInset = $.NSMakeSize(8, 8);
	text.verticallyResizable = true;
	text.autoresizingMask = $.NSViewWidthSizable;
	scroll.documentView = text;
	return { scroll, text };
}

// osascript has no menu bar, and Cmd-V, Cmd-C and the rest only reach the text
// box through an Edit menu's key equivalents. Without this, Wispr's paste (a
// Cmd-V) lands nowhere while typed keys still work.
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
	const editItem = $.NSMenuItem.alloc.init;
	editItem.submenu = edit;
	const menuBar = $.NSMenu.alloc.init;
	menuBar.addItem(editItem);
	app.mainMenu = menuBar;
}

// Return in the text box presses Send, rather than starting a new line.
// Option-Return still adds one (insertNewlineIgnoringFieldEditor:).
let sendButton = null;
let returnSends = null;
ObjC.registerSubclass({
	name: "WisprReturnSends",
	superclass: "NSObject",
	methods: {
		"textView:doCommandBySelector:": {
			types: ["bool", ["id", "SEL"]],
			implementation: (textView, selector) => {
				if (selector !== "insertNewline:") return false;
				sendButton.performClick(null);
				return true;
			},
		},
	},
});

function run(argv) {
	const attached = argv[0] || "";
	const clickedAt = Number(argv[1]) || Date.now();
	const app = $.NSApplication.sharedApplication;
	app.setActivationPolicy($.NSApplicationActivationPolicyAccessory);
	addEditMenu(app);

	const screen = $.NSScreen.mainScreen.visibleFrame.size;
	const box = textBox(Math.round(screen.width * WIDTH_SHARE), Math.round(screen.height * HEIGHT_SHARE));

	const alert = $.NSAlert.alloc.init;
	alert.messageText = "Wispr";
	alert.informativeText = `Sends with: ${attached || "nothing else"}\nDictate, then choose where it goes. Return sends to Larry.`;
	alert.icon = $.NSWorkspace.sharedWorkspace.iconForFile("/Applications/Wispr Flow.app");
	for (const title of BUTTONS) alert.addButtonWithTitle(title);
	sendButton = alert.buttons.objectAtIndex(0);
	returnSends = $.WisprReturnSends.alloc.init; // global: the delegate is a weak reference
	box.text.setDelegate(returnSends);
	alert.accessoryView = box.scroll;
	alert.layout;
	alert.window.initialFirstResponder = box.text;

	app.activateIgnoringOtherApps(true);
	console.log(`dialog: shown ${Date.now() - clickedAt} ms after the click`);
	const index = alert.runModal - $.NSAlertFirstButtonReturn;
	const button = BUTTONS[index];
	if (button === undefined || button === "Cancel") return "";
	return `${button}\n${box.text.string.js}`;
}
