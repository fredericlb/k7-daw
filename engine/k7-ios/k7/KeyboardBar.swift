// KeyboardBar.swift — takes UIKit's keyboard bar (the accessory and the shortcut bar) off a WKWebView on a Mac, where the
// iPad app draws that bar as a grey band over the page's bottom while the window has focus. What a reader gets wrong:
// the bar is the WebKit content view's, not the WKWebView's, so it is that inner view whose class is swapped.
// See engine/k7-runtime/docs/ios.md, "k7 on a Mac: the iPad app".

import ObjectiveC
import UIKit
import WebKit

enum KeyboardBar {
    private static let suffix = "_K7NoKeyboardBar"

    /// Hides the bar of `webView` on a Mac only; harmless to call twice, and a no-op if WebKit renames its content view.
    static func hide(in webView: WKWebView) {
        // A phone keeps its bar: its Done is how the keyboard goes away.
        guard ProcessInfo.processInfo.isiOSAppOnMac else { return }
        guard let content = webView.scrollView.subviews.first(where: {
            String(describing: type(of: $0)).hasPrefix("WKContent")
        }) else { return }
        content.inputAssistantItem.leadingBarButtonGroups = []
        content.inputAssistantItem.trailingBarButtonGroups = []
        guard let base = object_getClass(content) else { return }
        let name = String(cString: class_getName(base))
        if name.hasSuffix(suffix) { return }
        let subName = name + suffix
        var sub: AnyClass? = NSClassFromString(subName)
        if sub == nil, let made = objc_allocateClassPair(base, subName, 0) {
            let none: @convention(block) (AnyObject) -> UIView? = { _ in nil }
            let sel = #selector(getter: UIResponder.inputAccessoryView)
            let types = method_getTypeEncoding(class_getInstanceMethod(UIView.self, sel)!)
            class_addMethod(made, sel, imp_implementationWithBlock(none), types)
            objc_registerClassPair(made)
            sub = made
        }
        if let sub { object_setClass(content, sub) }
    }
}
