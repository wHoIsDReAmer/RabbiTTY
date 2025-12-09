#![allow(non_upper_case_globals)]

//! Windows-specific window customization
//! - Custom frame with WM_NCCALCSIZE to remove title bar but keep resize border

use iced::window::raw_window_handle::RawWindowHandle;
use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXSIZEFRAME, SM_CYSIZEFRAME, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetWindowPos, WM_NCCALCSIZE,
};

const SUBCLASS_ID: usize = 1;

#[repr(C)]
struct NcCalcSizeParams {
    rgrc: [windows::Win32::Foundation::RECT; 3],
    lppos: *mut c_void,
}

/// Subclass procedure to handle WM_NCCALCSIZE
/// This removes the title bar while keeping the resize border
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

    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Apply custom frame to window (remove title bar, keep resize border)
pub fn apply_style(handle: iced::window::raw_window_handle::WindowHandle<'_>) {
    let raw_handle = handle.as_raw();

    if let RawWindowHandle::Win32(win32_handle) = raw_handle {
        let hwnd = HWND(win32_handle.hwnd.get() as *mut _);

        unsafe {
            // Install subclass to intercept WM_NCCALCSIZE
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
