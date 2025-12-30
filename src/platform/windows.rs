#![allow(non_upper_case_globals)]

//! Windows-specific window customization
//! - Custom frame with WM_NCCALCSIZE to remove title bar but keep resize border
//! - WM_NCHITTEST to enable top edge resizing

use iced::window::raw_window_handle::RawWindowHandle;
use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetSystemMetrics, SM_CXSIZEFRAME, SM_CYSIZEFRAME, SWP_FRAMECHANGED, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SetWindowPos, WM_NCCALCSIZE, WM_NCHITTEST,
};

const SUBCLASS_ID: usize = 1;
const RESIZE_BORDER: i32 = 6; // Pixels for resize detection at top edge

#[repr(C)]
struct NcCalcSizeParams {
    rgrc: [windows::Win32::Foundation::RECT; 3],
    lppos: *mut c_void,
}

// Hit test return values
const HTCLIENT: isize = 1;
const HTLEFT: isize = 10;
const HTRIGHT: isize = 11;
const HTTOP: isize = 12;
const HTTOPLEFT: isize = 13;
const HTTOPRIGHT: isize = 14;
const HTBOTTOM: isize = 15;
const HTBOTTOMLEFT: isize = 16;
const HTBOTTOMRIGHT: isize = 17;

/// Subclass procedure to handle WM_NCCALCSIZE and WM_NCHITTEST
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    _dwrefdata: usize,
) -> LRESULT {
    if msg == WM_NCCALCSIZE && wparam.0 != 0 {
        let params = lparam.0 as *mut NcCalcSizeParams;
        if !params.is_null() {
            unsafe {
                let border_x = GetSystemMetrics(SM_CXSIZEFRAME);
                let border_y = GetSystemMetrics(SM_CYSIZEFRAME);

                (*params).rgrc[0].left += border_x;
                (*params).rgrc[0].right -= border_x;
                (*params).rgrc[0].bottom -= border_y;
            }
        }
        return LRESULT(0);
    }

    // Enable top edge resizing
    if msg == WM_NCHITTEST {
        // Get cursor position
        let x = (lparam.0 & 0xFFFF) as i16 as i32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

        unsafe {
            // Convert screen coordinates to client coordinates
            let mut pt = POINT { x, y };
            let _ = ScreenToClient(hwnd, &mut pt);

            // Get client rect
            let mut rect = std::mem::zeroed();
            let _ = GetClientRect(hwnd, &mut rect);

            let left = pt.x >= 0 && pt.x < RESIZE_BORDER;
            let right = pt.x >= rect.right - RESIZE_BORDER && pt.x < rect.right;
            let top = pt.y >= 0 && pt.y < RESIZE_BORDER;
            let bottom = pt.y >= rect.bottom - RESIZE_BORDER && pt.y < rect.bottom;

            if top && left {
                return LRESULT(HTTOPLEFT);
            }
            if top && right {
                return LRESULT(HTTOPRIGHT);
            }
            if bottom && left {
                return LRESULT(HTBOTTOMLEFT);
            }
            if bottom && right {
                return LRESULT(HTBOTTOMRIGHT);
            }
            if top {
                return LRESULT(HTTOP);
            }
            if bottom {
                return LRESULT(HTBOTTOM);
            }
            if left {
                return LRESULT(HTLEFT);
            }
            if right {
                return LRESULT(HTRIGHT);
            }
        }

        // Let default handling for other areas
        return LRESULT(HTCLIENT);
    }

    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Apply custom frame to window (remove title bar, keep resize border)
pub fn apply_style(handle: iced::window::raw_window_handle::WindowHandle<'_>) {
    let raw_handle = handle.as_raw();

    if let RawWindowHandle::Win32(win32_handle) = raw_handle {
        let hwnd = HWND(win32_handle.hwnd.get() as *mut _);

        unsafe {
            // Install subclass to intercept WM_NCCALCSIZE and WM_NCHITTEST
            let _ = SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0);

            // Force recalculation of non-client area immediately
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }
}
