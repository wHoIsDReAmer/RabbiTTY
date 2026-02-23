use crate::config::ThemeConfig;
use iced::window::raw_window_handle::{RawWindowHandle, WindowHandle};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
};
use objc2_foundation::MainThreadMarker;

pub fn apply_style(handle: WindowHandle<'_>, theme: &ThemeConfig) {
    apply_style_inner(handle, theme);
}

fn apply_style_inner(handle: WindowHandle<'_>, theme: &ThemeConfig) {
    if !theme.blur_enabled {
        return;
    }

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
    blur.setMaterial(macos_material_from_str(&theme.macos_blur_material));
    blur.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    blur.setState(NSVisualEffectState::Active);
    blur.setAlphaValue(f64::from(theme.macos_blur_alpha));
    blur.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    superview.addSubview_positioned_relativeTo(&blur, NSWindowOrderingMode::Below, Some(view));
}

fn macos_material_from_str(value: &str) -> NSVisualEffectMaterial {
    match value {
        "titlebar" => NSVisualEffectMaterial::Titlebar,
        "selection" => NSVisualEffectMaterial::Selection,
        "menu" => NSVisualEffectMaterial::Menu,
        "popover" => NSVisualEffectMaterial::Popover,
        "sidebar" => NSVisualEffectMaterial::Sidebar,
        "headerview" => NSVisualEffectMaterial::HeaderView,
        "sheet" => NSVisualEffectMaterial::Sheet,
        "windowbackground" => NSVisualEffectMaterial::WindowBackground,
        "hudwindow" => NSVisualEffectMaterial::HUDWindow,
        "fullscreenui" => NSVisualEffectMaterial::FullScreenUI,
        "tooltip" => NSVisualEffectMaterial::ToolTip,
        "contentbackground" => NSVisualEffectMaterial::ContentBackground,
        "underwindowbackground" => NSVisualEffectMaterial::UnderWindowBackground,
        "underpagebackground" => NSVisualEffectMaterial::UnderPageBackground,
        _ => NSVisualEffectMaterial::Sidebar,
    }
}
