// What mac/wispr_hands_free.sh needs to know about the front app, in a few ms
// rather than System Events' ~250 ms:
//
//   <bundle id>|<focused element role>|<focused window subrole>
//
// Prints nothing and exits 1 without Accessibility access, so the caller falls
// back to System Events. Built on demand into ~/.cache by the caller.

import AppKit
import ApplicationServices

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
}

func string(_ element: CFTypeRef?, _ name: String) -> String {
    guard let element, CFGetTypeID(element) == AXUIElementGetTypeID() else { return "none" }
    return attribute(element as! AXUIElement, name) as? String ?? "none"
}

guard AXIsProcessTrusted() else { exit(1) }
guard let front = NSWorkspace.shared.frontmostApplication else { exit(1) }
let app = AXUIElementCreateApplication(front.processIdentifier)
let role = string(attribute(app, kAXFocusedUIElementAttribute), kAXRoleAttribute)
let subrole = string(attribute(app, kAXFocusedWindowAttribute), kAXSubroleAttribute)
print("\(front.bundleIdentifier ?? "none")|\(role)|\(subrole)")
