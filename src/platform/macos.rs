use crate::config::ThemeConfig;
use iced::window::raw_window_handle::{DisplayHandle, RawWindowHandle, WindowHandle};
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSColor, NSImage, NSView, NSWindowCollectionBehavior};
use objc2_foundation::NSData;
use std::sync::OnceLock;

// macOS private CoreGraphics SPI for window background blur.
unsafe extern "C" {
    fn CGSSetWindowBackgroundBlurRadius(
        connection: *mut std::ffi::c_void,
        window_number: isize,
        radius: i32,
    ) -> i32;
    fn CGSDefaultConnectionForThread() -> *mut std::ffi::c_void;
}

pub fn apply_style(handle: WindowHandle<'_>, _display: DisplayHandle<'_>, theme: &ThemeConfig) {
    set_app_icon_once();
    apply_style_inner(handle, theme);
}

/// Play the macOS system beep.
pub fn ring_bell() {
    objc2_app_kit::NSBeep();
}

const APP_ICON_PNG: &[u8] = include_bytes!("../../assets/logo.png");

pub fn set_app_icon_once() {
    static APPLIED: OnceLock<()> = OnceLock::new();
    if APPLIED.get().is_some() {
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(APP_ICON_PNG);
    let image = NSImage::initWithData(NSImage::alloc(), &data);
    if let Some(image) = image {
        let app = NSApplication::sharedApplication(mtm);
        unsafe { app.setApplicationIconImage(Some(&image)) };
        let _ = APPLIED.set(());
    }
}

fn apply_style_inner(handle: WindowHandle<'_>, theme: &ThemeConfig) {
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };

    let view: &NSView = unsafe { appkit.ns_view.cast().as_ref() };
    let Some(window) = view.window() else {
        return;
    };

    // Prevent macOS from treating the title bar / content area as a drag
    // handle.  We manage window dragging explicitly via iced's window::drag
    // on the tab bar's empty space instead.
    window.setMovable(false);
    window.setMovableByWindowBackground(false);

    // Prevent macOS from tiling/fullscreening the window when dragging
    // near screen edges (macOS Sequoia+).
    let behavior = window.collectionBehavior();
    window.setCollectionBehavior(
        (behavior | NSWindowCollectionBehavior::FullScreenDisallowsTiling)
            - NSWindowCollectionBehavior::FullScreenAllowsTiling,
    );

    if !theme.blur_enabled {
        return;
    }

    // Make window non opaque with clear background so blur shows through
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));

    // Force the CAMetalLayer (wgpu surface) to be non-opaque
    if let Some(layer) = view.layer() {
        layer.setOpaque(false);
    }

    // Apply background blur using CoreGraphics
    let blur_radius = theme.macos_blur_radius;
    let window_number = window.windowNumber();
    unsafe {
        let connection = CGSDefaultConnectionForThread();
        CGSSetWindowBackgroundBlurRadius(connection, window_number, blur_radius);
    }
}
