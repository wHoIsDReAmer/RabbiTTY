use iced::window::raw_window_handle::{RawWindowHandle, WindowHandle};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
};
use objc2_foundation::MainThreadMarker;

pub fn apply_style(handle: WindowHandle<'_>) {
    apply_style_inner(handle);
}

fn apply_style_inner(handle: WindowHandle<'_>) {
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    let view: &NSView = unsafe { appkit.ns_view.cast().as_ref() };
    let Some(window) = view.window() else {
        return;
    };

    let Some(content_view) = window.contentView() else {
        return;
    };

    if !std::ptr::eq(content_view.as_ref(), view) {
        return;
    }

    let Some(superview) = (unsafe { view.superview() }) else {
        return;
    };

    let blur = NSVisualEffectView::new(mtm);
    blur.setFrame(view.frame());
    blur.setMaterial(NSVisualEffectMaterial::Sidebar);
    blur.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    blur.setState(NSVisualEffectState::Active);
    blur.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    superview.addSubview_positioned_relativeTo(&blur, NSWindowOrderingMode::Below, Some(view));
}
