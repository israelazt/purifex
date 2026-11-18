//! The window of the interface: a Win32 window that turns the system's messages
//! into `UiEvent`s, asks the program for a picture when it must draw, and draws
//! it with Direct2D. It runs on messages only (`GetMessage` waits), so a window
//! nobody touches costs no CPU. It knows nothing of the application.

use std::ffi::c_void;

use purifex_domain::ui_event::{Key, Reaction, UiEvent, UiHandler};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
    DWMWA_USE_IMMERSIVE_DARK_MODE,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForSystem, GetDpiForWindow, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
    GetWindowLongPtrW, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassExW, SetCursor,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, CREATESTRUCTW, CS_HREDRAW,
    CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, HTCLIENT, IDC_ARROW, IDC_HAND, MINMAXINFO, MSG,
    SWP_NOACTIVATE, SWP_NOZORDER, SW_SHOW, WM_APP, WM_CHAR, WM_CREATE, WM_DESTROY, WM_DPICHANGED,
    WM_ERASEBKGND, WM_GETMINMAXINFO, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCCREATE,
    WM_PAINT, WM_SETCURSOR, WM_SIZE, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
};

use crate::gfx::{Drawn, Renderer};

/// `WM_MOUSELEAVE`: the pointer left the window (asked for with `TrackMouseEvent`).
const WM_MOUSELEAVE: u32 = 0x02A3;
/// The smallest the window may be, in units.
const MIN_SIZE: (f32, f32) = (1000.0, 640.0);
/// Units scrolled by one notch of the wheel.
const WHEEL_NOTCH: f32 = 64.0;
/// The window's colors, as `0x00BBGGRR`: the caption matches the page.
const CAPTION: u32 = 0x000b_0807;
const CAPTION_TEXT: u32 = 0x00f2_f2f2;
const BORDER: u32 = 0x0026_201e;

struct State<H: UiHandler> {
    handler: *mut H,
    renderer: Option<Renderer>,
    dpi: u32,
    tracking_leave: bool,
}

fn low_word(value: isize) -> i32 {
    i32::from(value as i16)
}

fn high_word(value: isize) -> i32 {
    i32::from((value >> 16) as i16)
}

impl<H: UiHandler> State<H> {
    fn scale(&self) -> f32 {
        self.dpi as f32 / 96.0
    }

    fn handler(&mut self) -> &mut H {
        // SAFETY: the pointer came from the `&mut H` that `run` holds for as long
        // as the window exists, and messages are handled one at a time.
        unsafe { &mut *self.handler }
    }

    fn units(&self, px: i32) -> f32 {
        px as f32 / self.scale()
    }

    fn react(&mut self, hwnd: HWND, reaction: Reaction) {
        if reaction.redraw {
            // SAFETY: invalidates the whole client area of a live window.
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
        }
        if reaction.tick {
            // SAFETY: posts a message to a live window.
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_APP, WPARAM(0), LPARAM(0));
            }
        }
    }

    fn client(&self, hwnd: HWND) -> (u32, u32) {
        let mut rect = RECT::default();
        // SAFETY: the output is a live local.
        let _ = unsafe { GetClientRect(hwnd, &mut rect) };
        (
            (rect.right - rect.left).max(1) as u32,
            (rect.bottom - rect.top).max(1) as u32,
        )
    }

    fn paint(&mut self, hwnd: HWND) {
        let (w, h) = self.client(hwnd);
        if self.renderer.is_none() {
            match Renderer::for_window(hwnd, w, h, self.dpi as f32) {
                Ok(renderer) => self.renderer = Some(renderer),
                Err(error) => {
                    purifex_domain::log_error!("{error}");
                    return;
                }
            }
        }
        let scale = self.scale();
        // The renderer is taken out while the handler draws, so the handler can
        // measure text with it without two borrows of the state.
        let Some(mut renderer) = self.renderer.take() else {
            return;
        };
        let list = self
            .handler()
            .frame(w as f32 / scale, h as f32 / scale, renderer.text());
        match renderer.draw(&list) {
            Ok(Drawn::Done) => self.renderer = Some(renderer),
            Ok(Drawn::Recreate) => {
                // The device was lost: the next paint builds a new renderer.
                self.react(
                    hwnd,
                    Reaction {
                        redraw: true,
                        tick: false,
                    },
                );
            }
            Err(error) => {
                purifex_domain::log_error!("{error}");
                self.renderer = Some(renderer);
            }
        }
    }

    fn message(&mut self, hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        match msg {
            WM_ERASEBKGND => Some(LRESULT(1)),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                // SAFETY: paired Begin/EndPaint on a live window.
                unsafe {
                    BeginPaint(hwnd, &mut ps);
                    self.paint(hwnd);
                    let _ = EndPaint(hwnd, &ps);
                }
                Some(LRESULT(0))
            }
            WM_SIZE => {
                let (w, h) = self.client(hwnd);
                if let Some(renderer) = &self.renderer {
                    renderer.resize(w, h);
                }
                self.react(
                    hwnd,
                    Reaction {
                        redraw: true,
                        tick: false,
                    },
                );
                Some(LRESULT(0))
            }
            WM_DPICHANGED => {
                self.dpi = (wp.0 & 0xffff) as u32;
                if let Some(renderer) = &self.renderer {
                    renderer.set_dpi(self.dpi as f32);
                }
                // SAFETY: the system passes the suggested rectangle in `lp`.
                unsafe {
                    let suggested = &*(lp.0 as *const RECT);
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                Some(LRESULT(0))
            }
            WM_GETMINMAXINFO => {
                // SAFETY: the system passes a `MINMAXINFO` in `lp`.
                unsafe {
                    let info = &mut *(lp.0 as *mut MINMAXINFO);
                    let mut rect = RECT {
                        left: 0,
                        top: 0,
                        right: (MIN_SIZE.0 * self.scale()) as i32,
                        bottom: (MIN_SIZE.1 * self.scale()) as i32,
                    };
                    let _ = AdjustWindowRectExForDpi(
                        &mut rect,
                        WS_OVERLAPPEDWINDOW,
                        false,
                        Default::default(),
                        self.dpi,
                    );
                    info.ptMinTrackSize.x = rect.right - rect.left;
                    info.ptMinTrackSize.y = rect.bottom - rect.top;
                }
                Some(LRESULT(0))
            }
            WM_MOUSEMOVE => {
                if !self.tracking_leave {
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    // SAFETY: the structure is a live local with its size set.
                    self.tracking_leave = unsafe { TrackMouseEvent(&mut track) }.is_ok();
                }
                let event = UiEvent::Move {
                    x: self.units(low_word(lp.0)),
                    y: self.units(high_word(lp.0)),
                };
                let reaction = self.handler().event(event);
                self.react(hwnd, reaction);
                Some(LRESULT(0))
            }
            WM_MOUSELEAVE => {
                self.tracking_leave = false;
                let reaction = self.handler().event(UiEvent::Leave);
                self.react(hwnd, reaction);
                Some(LRESULT(0))
            }
            WM_LBUTTONDOWN => {
                let event = UiEvent::Press {
                    x: self.units(low_word(lp.0)),
                    y: self.units(high_word(lp.0)),
                };
                let reaction = self.handler().event(event);
                self.react(hwnd, reaction);
                Some(LRESULT(0))
            }
            WM_MOUSEWHEEL => {
                let notches = high_word(wp.0 as isize) as f32 / 120.0;
                let reaction = self.handler().event(UiEvent::Wheel {
                    dy: -notches * WHEEL_NOTCH,
                });
                self.react(hwnd, reaction);
                Some(LRESULT(0))
            }
            WM_CHAR => {
                let event = match char::from_u32(wp.0 as u32) {
                    Some('\u{8}') => Some(UiEvent::Key(Key::Backspace)),
                    Some('\u{1b}') => Some(UiEvent::Key(Key::Escape)),
                    Some('\r') => Some(UiEvent::Key(Key::Enter)),
                    Some('\u{b}') => Some(UiEvent::Key(Key::CtrlK)),
                    Some(c) if !c.is_control() => Some(UiEvent::Char(c)),
                    _ => None,
                };
                if let Some(event) = event {
                    let reaction = self.handler().event(event);
                    self.react(hwnd, reaction);
                }
                Some(LRESULT(0))
            }
            WM_APP => {
                let reaction = self.handler().tick();
                self.react(hwnd, reaction);
                Some(LRESULT(0))
            }
            WM_SETCURSOR if (lp.0 & 0xffff) as u32 == HTCLIENT => {
                let cursor = if self.handler().hand_cursor() {
                    IDC_HAND
                } else {
                    IDC_ARROW
                };
                // SAFETY: loads a stock cursor and sets it.
                unsafe { SetCursor(LoadCursorW(None, cursor).ok()) };
                Some(LRESULT(1))
            }
            WM_DESTROY => {
                // SAFETY: ends the message loop of this thread.
                unsafe { PostQuitMessage(0) };
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}

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
        }));
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
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        drop(Box::from_raw(state));
    }
    Ok(())
}
