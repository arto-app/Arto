import Cocoa
import QuickLookUI
import WebKit

/// QuickLook preview extension principal class.
///
/// Renders a Markdown file to a fully self-contained HTML document (produced by
/// the Rust `arto_page` static library) and displays it in a `WKWebView`. The
/// extension runs sandboxed, so the HTML embeds all CSS/JS/fonts inline — it
/// never reads sibling files or the network.
@objc(ArtoPreviewViewController)
final class ArtoPreviewViewController: NSViewController, QLPreviewingController, WKNavigationDelegate {
    private var webView: WKWebView!
    private var completion: ((Error?) -> Void)?
    private var didAllowInitialLoad = false

    override func loadView() {
        let configuration = WKWebViewConfiguration()
        // Use an ephemeral (in-memory) data store. The default persistent store
        // makes WKWebView probe an on-disk storage directory during init, which
        // the extension's sandbox denies — leaving the WebView unable to load
        // and the Quick Look preview stuck spinning forever. An untrusted
        // preview also has no reason to persist cookies/cache/localStorage.
        configuration.websiteDataStore = .nonPersistent()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = true
        webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = self
        self.view = webView
    }

    func preparePreviewOfFile(
        at url: URL,
        completionHandler handler: @escaping (Error?) -> Void
    ) {
        // The Rust function returns a Rust-allocated C string (release it only
        // with arto_page_free_string, never libc free); it stays valid until we
        // free it, so returning it out of the closure is safe.
        let htmlPointer = url.withUnsafeFileSystemRepresentation { representation in
            representation.flatMap { arto_page_render_markdown_file($0) }
        }
        guard let htmlPointer else {
            handler(ArtoPreviewError.renderFailed)
            return
        }
        defer { arto_page_free_string(htmlPointer) }

        let html = String(cString: htmlPointer)
        completion = handler
        didAllowInitialLoad = false
        // Signal completion once the DOM has loaded (didFinish); the embedded
        // JS then renders mermaid/KaTeX/highlight in the live view.
        webView.loadHTMLString(html, baseURL: nil)
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        guard didAllowInitialLoad else {
            // The initial in-memory document load.
            didAllowInitialLoad = true
            decisionHandler(.allow)
            return
        }
        // Allow same-document navigations (in-page `#fragment` anchor/TOC links,
        // which stay on the preview content); cancel anything else, e.g. an
        // external link that would leave the sandboxed preview.
        if let target = navigationAction.request.url,
            let current = webView.url,
            documentPart(of: target) == documentPart(of: current) {
            decisionHandler(.allow)
        } else {
            decisionHandler(.cancel)
        }
    }

    /// The URL string without its `#fragment`, used to detect same-document
    /// (anchor-only) navigations.
    private func documentPart(of url: URL) -> String {
        let string = url.absoluteString
        if let hash = string.firstIndex(of: "#") {
            return String(string[..<hash])
        }
        return string
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        completion?(nil)
        completion = nil
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        completion?(error)
        completion = nil
    }

    func webView(
        _ webView: WKWebView,
        didFailProvisionalNavigation navigation: WKNavigation!,
        withError error: Error
    ) {
        completion?(error)
        completion = nil
    }
}

enum ArtoPreviewError: Error {
    case renderFailed
}
