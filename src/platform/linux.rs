//! Linux window customization.
//!
//! Background blur on Linux is performed by the compositor, not the
//! application, and the mechanism differs per windowing system:
//!
//! * **X11** — set the `_KDE_NET_WM_BLUR_BEHIND_REGION` property on the
//!   window. KWin honours it directly and picom honours it when
//!   `blur-background` is enabled. An empty region blurs the whole window.
//! * **Wayland** — request blur through KWin's `org_kde_kwin_blur`
//!   protocol, binding it onto winit's existing `wl_surface`.
//!
//! Either way the window must be translucent (`background_opacity < 1.0`)
//! for the blur to be visible, which the renderer already honours. Other
//! compositors (Mutter, plain wlroots) simply ignore the request.
//!
//! Blur is read once at startup; toggling it in Settings takes effect on
//! the next launch.

use crate::config::ThemeConfig;
use iced::window::raw_window_handle::{
    DisplayHandle, RawDisplayHandle, RawWindowHandle, WindowHandle,
};

pub fn apply_style(window: WindowHandle<'_>, display: DisplayHandle<'_>, theme: &ThemeConfig) {
    match (window.as_raw(), display.as_raw()) {
        (RawWindowHandle::Xlib(win), _) => apply_x11_blur(win.window as u32, theme.blur_enabled),
        (RawWindowHandle::Xcb(win), _) => apply_x11_blur(win.window.get(), theme.blur_enabled),
        (RawWindowHandle::Wayland(win), RawDisplayHandle::Wayland(dpy)) if theme.blur_enabled => {
            // SAFETY: the pointers come straight from winit's live window and
            // display and remain valid for the duration of this call, which
            // runs synchronously inside the winit event loop.
            unsafe { apply_wayland_blur(win.surface.as_ptr(), dpy.display.as_ptr()) };
        }
        _ => {}
    }
}

/// No system bell API without extra dependencies; stay silent.
pub fn ring_bell() {}

// ── X11 (KWin / picom): _KDE_NET_WM_BLUR_BEHIND_REGION ───────────────
fn apply_x11_blur(window: u32, enabled: bool) {
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
    use x11rb::wrapper::ConnectionExt as _;

    // Open our own connection to the same `$DISPLAY`; the property we set
    // is stored server-side on the window and outlives this connection.
    let Ok((conn, _screen)) = x11rb::connect(None) else {
        return;
    };

    let Ok(cookie) = conn.intern_atom(false, b"_KDE_NET_WM_BLUR_BEHIND_REGION") else {
        return;
    };
    let Ok(reply) = cookie.reply() else {
        return;
    };
    let atom = reply.atom;

    if enabled {
        // Empty region = blur the entire window.
        let _ = conn.change_property32(PropMode::REPLACE, window, atom, AtomEnum::CARDINAL, &[]);
    } else {
        let _ = conn.delete_property(window, atom);
    }
    let _ = conn.flush();
}

// ── Wayland (KWin): org_kde_kwin_blur ────────────────────────────────
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    backend::{Backend, ObjectId},
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry::WlRegistry, wl_surface::WlSurface},
};
use wayland_protocols_plasma::blur::client::{
    org_kde_kwin_blur::OrgKdeKwinBlur, org_kde_kwin_blur_manager::OrgKdeKwinBlurManager,
};

struct BlurState;

impl Dispatch<WlRegistry, GlobalListContents> for BlurState {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

// The blur manager and blur objects emit no events.
wayland_client::delegate_noop!(BlurState: OrgKdeKwinBlurManager);
wayland_client::delegate_noop!(BlurState: OrgKdeKwinBlur);

unsafe fn apply_wayland_blur(
    surface_ptr: *mut std::ffi::c_void,
    display_ptr: *mut std::ffi::c_void,
) {
    // Wrap winit's existing wl_display without taking ownership of it.
    let backend = unsafe { Backend::from_foreign_display(display_ptr.cast()) };
    let conn = Connection::from_backend(backend);

    // Wrap winit's existing wl_surface as a proxy on this connection.
    let Ok(surface_id) =
        (unsafe { ObjectId::from_ptr(WlSurface::interface(), surface_ptr.cast()) })
    else {
        return;
    };
    let Ok(surface) = WlSurface::from_id(&conn, surface_id) else {
        return;
    };

    let Ok((globals, queue)) = registry_queue_init::<BlurState>(&conn) else {
        return;
    };
    let qh = queue.handle();

    // Bind the KWin blur manager; absent on compositors without blur support.
    let Ok(manager) = globals.bind::<OrgKdeKwinBlurManager, _, _>(&qh, 1..=1, ()) else {
        return;
    };

    let blur = manager.create(&surface, &qh, ());
    blur.set_region(None); // whole surface
    blur.commit();
    // Applied on winit's next surface commit; just flush our requests out.
    let _ = conn.flush();
}
