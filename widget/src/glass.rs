//! Frosted-glass window: Windows' acrylic blur-behind plus the system's own rounded corners, which is the
//! only thing that clips acrylic (it ignores the window region). The corner radius is therefore fixed at
//! Windows 11's 8px; the app draws its shell with the same radius while glass is on.
//! ponytail: blur only; the refraction/chromatic fringing of web "liquid glass" would need screen capture +
//! a shader every frame, which isn't worth the CPU for a lightweight widget.
#![allow(non_snake_case)]

/// The corner radius Windows 11 gives a window with DWMWCP_ROUND, in points at 100% scale.
pub const SYSTEM_RADIUS: f32 = 8.0;

#[cfg(windows)]
mod sys {
    use std::ffi::c_void;
    #[repr(C)]
    pub struct AccentPolicy { pub state: i32, pub flags: i32, pub gradient: u32, pub anim: i32 }
    #[repr(C)]
    pub struct WinCompAttr { pub attr: i32, pub data: *mut c_void, pub size: usize }
    #[link(name = "dwmapi")]
    unsafe extern "system" {
        pub fn DwmSetWindowAttribute(hwnd: isize, attr: u32, v: *const c_void, size: u32) -> i32;
        pub fn DwmExtendFrameIntoClientArea(hwnd: isize, margins: *const [i32; 4]) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        pub fn CreateRoundRectRgn(l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> isize;
        pub fn CreateRectRgn(l: i32, t: i32, r: i32, b: i32) -> isize;
        pub fn DeleteObject(o: isize) -> i32;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn SetWindowRgn(hwnd: isize, rgn: isize, redraw: i32) -> i32;
        pub fn GetWindowRgn(hwnd: isize, rgn: isize) -> i32;
        pub fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
        pub fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
        pub fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
        pub fn GetWindowRect(hwnd: isize, rect: *mut [i32; 4]) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn GetModuleHandleA(name: *const u8) -> isize;
        pub fn GetProcAddress(module: isize, name: *const u8) -> *const c_void;
    }
}

/// Turns glass on or off.
pub fn apply(hwnd: isize, on: bool) {
    #[cfg(windows)]
    unsafe {
        // DWMWA_WINDOW_CORNER_PREFERENCE: 1 = none. The card rounds itself (and the window is clipped to it);
        // the system's own rounding comes with a drop shadow that shows as a grey halo on bright backdrops
        let pref: i32 = 1;
        sys::DwmSetWindowAttribute(hwnd, 33, &pref as *const i32 as *const std::ffi::c_void, 4);
        // DWMWA_NCRENDERING_POLICY = DWMNCRP_DISABLED: no system frame, so no soft drop shadow around the card
        // (it is invisible on dark backdrops but shows as a grey halo on bright ones)
        let disabled: i32 = 1;
        sys::DwmSetWindowAttribute(hwnd, 2, &disabled as *const i32 as *const std::ffi::c_void, 4);
        // DWMWA_BORDER_COLOR = DWMWA_COLOR_NONE: no 1px system outline around the window (the card draws its own rim)
        let none: u32 = 0xFFFF_FFFE;
        sys::DwmSetWindowAttribute(hwnd, 34, &none as *const u32 as *const std::ffi::c_void, 4);
        // acrylic accent (4) with a faint tint; the card paints its own darker tint on top.
        // SetWindowCompositionAttribute is undocumented and has no import-library entry: looked up at runtime.
        let mut p = sys::AccentPolicy { state: if on { 4 } else { 0 }, flags: 2, gradient: 0x30_12_0d_0e, anim: 0 };
        let mut d = sys::WinCompAttr { attr: 19, data: &mut p as *mut _ as *mut std::ffi::c_void, size: std::mem::size_of::<sys::AccentPolicy>() };
        let f = sys::GetProcAddress(sys::GetModuleHandleA(b"user32.dll\0".as_ptr()), b"SetWindowCompositionAttribute\0".as_ptr());
        if !f.is_null() {
            let f: unsafe extern "system" fn(isize, *mut sys::WinCompAttr) -> i32 = std::mem::transmute(f);
            f(hwnd, &mut d);
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd, on);
}

/// Flat (no blur) mode: clips the window to the card's own rounded rectangle, so nothing Windows draws at
/// the window edge (border, shadow) can show outside it. Returns the window size in px; pass 0.0 to remove the clip.
pub fn clip(hwnd: isize, radius_px: f32) -> (i32, i32) {
    #[cfg(windows)]
    unsafe {
        if radius_px <= 0.0 { sys::SetWindowRgn(hwnd, 0, 1); return (0, 0) }
        let mut r = [0i32; 4];
        if sys::GetWindowRect(hwnd, &mut r) != 0 {
            let (w, h) = (r[2] - r[0], r[3] - r[1]);
            let d = (radius_px * 2.0) as i32;
            // +1: CreateRoundRectRgn's right/bottom edges are exclusive
            // top = 1: row 0 is a leftover system frame strip (gray/white 1px line); the card starts below it
            sys::SetWindowRgn(hwnd, sys::CreateRoundRectRgn(0, 1, w + 1, h + 1, d, d), 1);
            return (w, h);
        }
    }
    let _ = (hwnd, radius_px);
    (0, 0)
}

/// Windows draws a soft drop shadow (a grey halo on bright backdrops) for any window with a caption or thick
/// frame. Strip those style bits; call every frame, since the windowing layer can put them back.
pub fn strip_frame(hwnd: isize) {
    #[cfg(windows)]
    unsafe {
        const GWL_STYLE: i32 = -16;
        // WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX
        const FRAME: u32 = 0x00C0_0000 | 0x0004_0000 | 0x0008_0000 | 0x0002_0000 | 0x0001_0000;
        const WS_POPUP: u32 = 0x8000_0000;
        // the windowing layer draws the frameless window's shadow (and a 1px white top line) by extending the DWM
        // frame 1px into the window: put the margins back to zero
        sys::DwmExtendFrameIntoClientArea(hwnd, &[0, 0, 0, 0]);
        let st = sys::GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let want = (st & !FRAME) | WS_POPUP;
        if want != st {
            sys::SetWindowLongPtrW(hwnd, GWL_STYLE, want as i32 as isize);
            // SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED
            sys::SetWindowPos(hwnd, 0, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0004 | 0x0010 | 0x0020);
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd;
}

/// True when the window has lost its clip region (something in the windowing layer can reset it).
pub fn region_missing(hwnd: isize) -> bool {
    #[cfg(windows)]
    unsafe {
        let probe = sys::CreateRectRgn(0, 0, 0, 0);
        let kind = sys::GetWindowRgn(hwnd, probe); // 0 = ERROR: the window has no region
        sys::DeleteObject(probe);
        return kind == 0;
    }
    #[cfg(not(windows))]
    { let _ = hwnd; false }
}
