//! The window of the interface: a Win32 window that turns the system's messages
//! into `UiEvent`s, asks the program for a picture when it must draw, and draws
//! it with Direct2D. It runs on messages only (`GetMessage` waits), and while
//! something moves a timer asks for a picture about every 16 ms, so a window
//! nobody touches costs no CPU. It knows nothing of the application.

use std::ffi::c_void;
use std::time::Instant;

use purifex_domain::ui_event::UiHandler;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
    DWMWA_USE_IMMERSIVE_DARK_MODE,
};
use windows::Win32::Graphics::Gdi::UpdateWindow;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForSystem, GetDpiForWindow, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW, LoadCursorW,
    PostMessageW, RegisterClassExW, SetWindowLongPtrW, ShowWindow, SystemParametersInfoW,
    TranslateMessage, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA,
    IDC_ARROW, MSG, SPI_GETCLIENTAREAANIMATION, SW_SHOW, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    WM_APP, WM_CREATE, WM_NCCREATE, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};

/// The window's colors, as `0x00BBGGRR`: the caption matches the page.
const CAPTION: u32 = 0x000b_0807;
const CAPTION_TEXT: u32 = 0x00f2_f2f2;
const BORDER: u32 = 0x0026_201e;

mod state;

use state::State;

unsafe extern "system" fn procedure<H: UiHandler>(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        // SAFETY: on creation `lp` points to the `CREATESTRUCTW` that carries the state.
        let create = unsafe { &*(lp.0 as *const CREATESTRUCTW) };
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize) };
    }
    // SAFETY: the user data is the `State` that `run` boxed, until it is destroyed.
    let state = unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State<H>).as_mut() };
    if msg != WM_CREATE {
        if let Some(state) = state {
            if let Some(result) = state.message(hwnd, msg, wp, lp) {
                return result;
            }
        }
    }
    // SAFETY: the default handling of the system.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

/// Opens the window (`width` x `height` units) and runs until it is closed.
pub fn run<H: UiHandler>(
    title: &str,
    width: f32,
    height: f32,
    handler: &mut H,
) -> Result<(), String> {
    // SAFETY: the whole of window creation and the message loop, with live
    // arguments; the state outlives the window and is freed after the loop.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("PurifexWindow");
        let description = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(procedure::<H>),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassExW(&description) == 0 {
            return Err("no se pudo registrar la ventana".into());
        }
        let dpi = GetDpiForSystem();
        let scale = dpi as f32 / 96.0;
        let mut frame = RECT {
            left: 0,
            top: 0,
            right: (width * scale) as i32,
            bottom: (height * scale) as i32,
        };
        let _ = AdjustWindowRectExForDpi(
            &mut frame,
            WS_OVERLAPPEDWINDOW,
            false,
            Default::default(),
            dpi,
        );
        let state = Box::into_raw(Box::new(State {
            handler: handler as *mut H,
            renderer: None,
            dpi,
            tracking_leave: false,
            opened: Instant::now(),
            timer: false,
        }));
        // "Show animations in Windows" turned off: the interface does not move.
        let mut animations = windows::core::BOOL(1);
        let asked = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut animations as *mut _ as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        if asked.is_ok() && !animations.as_bool() {
            (*state).handler().set_reduced_motion(true);
        }
        let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd = CreateWindowExW(
            Default::default(),
            class,
            PCWSTR(wide.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            frame.right - frame.left,
            frame.bottom - frame.top,
            None,
            None,
            Some(instance.into()),
            Some(state as *const c_void),
        )
        .map_err(|e| format!("no se pudo crear la ventana: {e}"))?;
        (*state).dpi = GetDpiForWindow(hwnd).max(96);

        // A dark caption and border, as the page: a hint to Windows 11 that older
        // versions ignore.
        let dark: i32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const c_void,
            4,
        );
        for (attribute, value) in [
            (DWMWA_CAPTION_COLOR, CAPTION),
            (DWMWA_TEXT_COLOR, CAPTION_TEXT),
            (DWMWA_BORDER_COLOR, BORDER),
        ] {
            let _ = DwmSetWindowAttribute(hwnd, attribute, &value as *const _ as *const c_void, 4);
        }

        let _ = ShowWindow(hwnd, SW_SHOW);
        // The first picture goes before anything is read; then a tick, if the
        // program has something to read.
        let _ = UpdateWindow(hwnd);
        let _ = PostMessageW(Some(hwnd), WM_APP, WPARAM(0), LPARAM(0));
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        drop(Box::from_raw(state));
    }
    Ok(())
}
