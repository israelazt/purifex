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

mod icon;
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
    // SAFETY: the system calls of creating a window and running its message loop,
    // each with live arguments (see the functions); the state outlives the window and
    // is freed after the loop, or here if the window could not be created.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        register_class::<H>(instance)?;
        let dpi = GetDpiForSystem();
        let state = Box::into_raw(Box::new(State {
            handler: handler as *mut H,
            renderer: None,
            dpi,
            tracking_leave: false,
            opened: Instant::now(),
            timer: false,
        }));
        if animations_are_off() {
            (*state).handler().set_reduced_motion(true);
        }
        let frame = outer_frame(width, height, dpi);
        let hwnd = match create_window(title, instance, frame, state) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                drop(Box::from_raw(state));
                return Err(error);
            }
        };
        (*state).dpi = GetDpiForWindow(hwnd).max(96);
        style_caption(hwnd);
        icon::set(hwnd);
        let _ = ShowWindow(hwnd, SW_SHOW);
        // The first picture goes before anything is read; then a tick, if the
        // program has something to read.
        let _ = UpdateWindow(hwnd);
        let _ = PostMessageW(Some(hwnd), WM_APP, WPARAM(0), LPARAM(0));
        run_message_loop();
        drop(Box::from_raw(state));
    }
    Ok(())
}

/// Registers the class of the window, whose procedure is `procedure::<H>`.
///
/// # Safety
/// `instance` is the handle of this module.
unsafe fn register_class<H: UiHandler>(
    instance: windows::Win32::Foundation::HMODULE,
) -> Result<(), String> {
    let description = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(procedure::<H>),
        hInstance: instance.into(),
        // SAFETY: a stock cursor, no instance needed.
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
        lpszClassName: w!("PurifexWindow"),
        ..Default::default()
    };
    // SAFETY: `description` is fully initialised and outlives the call.
    if unsafe { RegisterClassExW(&description) } == 0 {
        return Err("no se pudo registrar la ventana".into());
    }
    Ok(())
}

/// "Show animations in Windows" turned off: the interface must not move.
fn animations_are_off() -> bool {
    let mut animations = windows::core::BOOL(1);
    // SAFETY: the output is a `BOOL`, as `SPI_GETCLIENTAREAANIMATION` writes.
    let asked = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut animations as *mut _ as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    asked.is_ok() && !animations.as_bool()
}

/// The size of the whole window (borders and caption included) that holds a client area
/// of `width` x `height` units at `dpi`.
fn outer_frame(width: f32, height: f32, dpi: u32) -> RECT {
    let scale = dpi as f32 / 96.0;
    let mut frame = RECT {
        left: 0,
        top: 0,
        right: (width * scale) as i32,
        bottom: (height * scale) as i32,
    };
    // SAFETY: `frame` is a live rectangle.
    let _ = unsafe {
        AdjustWindowRectExForDpi(
            &mut frame,
            WS_OVERLAPPEDWINDOW,
            false,
            Default::default(),
            dpi,
        )
    };
    frame
}

/// Creates the window; `state` is handed to the procedure on `WM_NCCREATE`.
///
/// # Safety
/// `state` points to a live `State<H>` that outlives the window.
unsafe fn create_window<H: UiHandler>(
    title: &str,
    instance: windows::Win32::Foundation::HMODULE,
    frame: RECT,
    state: *mut State<H>,
) -> Result<HWND, String> {
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: the class was registered; `wide` is NUL-terminated and outlives the call.
    unsafe {
        CreateWindowExW(
            Default::default(),
            w!("PurifexWindow"),
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
    }
    .map_err(|e| format!("no se pudo crear la ventana: {e}"))
}

/// A dark caption and border, as the page: a hint to Windows 11 that older versions
/// ignore.
fn style_caption(hwnd: HWND) {
    let dark: i32 = 1;
    // SAFETY: each value is a live 4-byte integer, the size the attributes take.
    unsafe {
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
    }
}

/// Waits for messages and hands each to the window's procedure, until the window closes.
fn run_message_loop() {
    let mut message = MSG::default();
    // SAFETY: `message` is a live `MSG`; the calls only read it and the window's state.
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
