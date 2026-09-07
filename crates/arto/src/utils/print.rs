//! Native print support for the current window's webview content.
//!
//! Rendering adjustments for paper output (hiding the app chrome, fitting
//! Mermaid diagrams and images into an A4 page, etc.) live in the renderer's
//! `@media print` stylesheet (`frontend/style/print.css`). The renderer is
//! also switched to the light theme while the print dialog is open (see
//! `window.Arto.print` in `frontend/src/main.ts`) so printed output is
//! always light regardless of the UI theme.

use std::path::PathBuf;

use dioxus::document;
use dioxus::prelude::spawn;

/// Open the native print dialog for the current window's webview.
///
/// On macOS this runs an `NSPrintOperation`, so the system dialog provides
/// "Save as PDF" for PDF export. `file` names the print job, which becomes
/// the default file name offered by "Save as PDF".
pub fn print_window(file: Option<PathBuf>) {
    spawn(async move {
        // Forcing the light theme for print (and restoring it afterwards)
        // only runs on macOS. There `run_print_dialog` awaits the print
        // sheet, so the light theme stays active until the content is
        // captured and can safely be restored once the sheet closes. On
        // other platforms `webview.print()` returns immediately with no
        // completion signal, so switching the theme would race the print
        // capture (restoring dark before the job is rendered); those
        // platforms rely on the print stylesheet instead, at the cost of
        // dark-baked Mermaid/code-block colors in the output.
        //
        // Either way the renderer has to draw what the reader never scrolled
        // to: diagrams, formulas and highlighting wait for the viewport, and
        // a print job has no viewport to wait for. Only the theme switch is
        // macOS-only; the drawing is not, and a page printed without it comes
        // out with blank diagram boxes and raw `$…$`.
        #[cfg(target_os = "macos")]
        let prepare = "window.Arto?.print?.prepare?.()";
        #[cfg(not(target_os = "macos"))]
        let prepare = "window.Arto?.print?.draw?.()";

        let mut eval = document::eval(&format!(
            r#"
            (async () => {{
                try {{
                    await {prepare};
                }} catch (e) {{
                    console.error("Print preparation failed:", e);
                }}
                dioxus.send(true);
            }})();
            "#
        ));
        let _ = eval.recv::<bool>().await;

        run_print_dialog(file).await;

        // Restore the theme that was active before printing.
        #[cfg(target_os = "macos")]
        let _ = document::eval("window.Arto?.print?.restore?.();");
    });
}

/// Run the print dialog and resolve once it has been closed.
#[cfg(target_os = "macos")]
async fn run_print_dialog(file: Option<PathBuf>) {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    macos::start_print_operation(file, tx);
    // Resolves when the dialog closes (delegate callback fires) or
    // immediately when the sender is dropped on a failure path.
    let _ = rx.await;
}

#[cfg(not(target_os = "macos"))]
async fn run_print_dialog(_file: Option<PathBuf>) {
    if let Err(e) = dioxus::desktop::window().webview.print() {
        tracing::warn!("Failed to open print dialog: {e}");
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::path::PathBuf;

    use dioxus_desktop::wry::WebViewExtMacOS;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{define_class, msg_send, sel, DeclaredClass, MainThreadOnly};
    use objc2_app_kit::{NSPrintInfo, NSPrintOperation, NSWindow};
    use objc2_foundation::{MainThreadMarker, NSString};
    use tokio::sync::oneshot;

    /// 36pt = 0.5in page margins on every side (wry's default would be 0,
    /// making content touch the paper edges).
    const MARGIN_PT: f64 = 36.0;

    struct PrintDelegateIvars {
        on_done: RefCell<Option<oneshot::Sender<()>>>,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "ArtoPrintDelegate"]
        #[thread_kind = MainThreadOnly]
        #[ivars = PrintDelegateIvars]
        struct PrintDelegate;

        impl PrintDelegate {
            #[unsafe(method(printOperationDidRun:success:contextInfo:))]
            fn print_operation_did_run(
                &self,
                _operation: *mut AnyObject,
                _success: bool,
                _context_info: *mut c_void,
            ) {
                if let Some(tx) = self.ivars().on_done.borrow_mut().take() {
                    let _ = tx.send(());
                }
            }
        }
    );

    impl PrintDelegate {
        fn new(mtm: MainThreadMarker, on_done: oneshot::Sender<()>) -> Retained<Self> {
            let this = mtm.alloc::<Self>().set_ivars(PrintDelegateIvars {
                on_done: RefCell::new(Some(on_done)),
            });
            // Safety: plain NSObject init
            unsafe { msg_send![super(this), init] }
        }
    }

    thread_local! {
        /// Keep delegates alive while their print dialog is open.
        ///
        /// `runOperationModalForWindow:` does not retain its delegate, and
        /// dropping the delegate inside its own callback would free the
        /// object while its method is still on the stack. Instead, finished
        /// delegates (their sender already consumed) are swept when the next
        /// print starts.
        static ACTIVE_DELEGATES: RefCell<Vec<Retained<PrintDelegate>>> =
            const { RefCell::new(Vec::new()) };
    }

    /// Start a print operation for the current window as a window sheet.
    ///
    /// WKWebView's `printOperationWithPrintInfo:` must be run modal for a
    /// window (Safari-style sheet); `runOperation` is not supported. The
    /// `on_done` sender fires when the sheet closes; on failure paths it is
    /// dropped, which the receiver observes as an error.
    pub(super) fn start_print_operation(file: Option<PathBuf>, on_done: oneshot::Sender<()>) {
        let Some(mtm) = MainThreadMarker::new() else {
            tracing::warn!("Print operation must be started on the main thread");
            return;
        };

        // Sweep delegates whose print dialog has already closed.
        ACTIVE_DELEGATES.with(|delegates| {
            delegates
                .borrow_mut()
                .retain(|d| d.ivars().on_done.borrow().is_some());
        });

        let webview = dioxus::desktop::window().webview.webview();

        // Safety: objc runtime calls are unsafe
        unsafe {
            let responds: bool =
                msg_send![&*webview, respondsToSelector: sel!(printOperationWithPrintInfo:)];
            if !responds {
                tracing::warn!("WKWebView does not support printOperationWithPrintInfo:");
                return;
            }

            let window: Option<Retained<NSWindow>> = msg_send![&*webview, window];
            let Some(window) = window else {
                tracing::warn!("Webview is not attached to a window; cannot print");
                return;
            };

            // Copy the shared print info so setting margins does not mutate
            // the process-wide singleton that other print operations rely on.
            let shared_print_info = NSPrintInfo::sharedPrintInfo();
            let print_info: Retained<NSPrintInfo> = msg_send![&*shared_print_info, copy];
            print_info.setTopMargin(MARGIN_PT);
            print_info.setRightMargin(MARGIN_PT);
            print_info.setBottomMargin(MARGIN_PT);
            print_info.setLeftMargin(MARGIN_PT);

            let operation: Retained<NSPrintOperation> =
                msg_send![&*webview, printOperationWithPrintInfo: &*print_info];

            // The job title becomes the default file name for "Save as PDF"
            // (without it, every PDF would be suggested as "Arto.pdf").
            if let Some(stem) = file.as_deref().and_then(|f| f.file_stem()) {
                let title = NSString::from_str(&stem.to_string_lossy());
                operation.setJobTitle(Some(&title));
            }

            // Allow the modal to detach from the current thread and be non-blocking
            operation.setCanSpawnSeparateThread(true);

            let delegate = PrintDelegate::new(mtm, on_done);
            ACTIVE_DELEGATES.with(|delegates| delegates.borrow_mut().push(delegate.clone()));

            let delegate_obj: &AnyObject = &delegate;
            operation.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
                &window,
                Some(delegate_obj),
                Some(sel!(printOperationDidRun:success:contextInfo:)),
                std::ptr::null_mut(),
            );
        }
    }
}
