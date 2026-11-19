//! The state of the window and how it answers each message of the system: it turns
//! them into `UiEvent`s for the program, paints when asked, and keeps a timer
//! running only while something moves.

use std::time::Instant;

use purifex_domain::ui_event::{Key, Reaction, UiEvent, UiHandler};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, InvalidateRect, UpdateWindow, PAINTSTRUCT,
};
use windows::Win32::UI::HiDpi::AdjustWindowRectExForDpi;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, KillTimer, LoadCursorW, PostMessageW, PostQuitMessage, SetCursor, SetTimer,
    SetWindowPos, HTCLIENT, IDC_ARROW, IDC_HAND, MINMAXINFO, SWP_NOACTIVATE, SWP_NOZORDER, WM_APP,
    WM_CHAR, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_SETCURSOR, WM_SETFOCUS,
    WM_SIZE, WM_TIMER, WS_OVERLAPPEDWINDOW,
};

use crate::gfx::{Drawn, Renderer};

/// `WM_MOUSELEAVE`: the pointer left the window (asked for with `TrackMouseEvent`).
const WM_MOUSELEAVE: u32 = 0x02A3;
/// The id of the timer that paces the animations.
const FRAME_TIMER: usize = 1;
/// The id of the timer that asks for the next tick while a background job goes on.
const TICK_TIMER: usize = 2;
/// How long after a tick the next one is asked for, in ms.
const TICK_EVERY: u32 = 50;
/// The smallest the window may be, in units.
const MIN_SIZE: (f32, f32) = (1000.0, 640.0);
/// Units scrolled by one notch of the wheel.
const WHEEL_NOTCH: f32 = 64.0;

pub(super) struct State<H: UiHandler> {
    pub(super) handler: *mut H,
    pub(super) renderer: Option<Renderer>,
    pub(super) dpi: u32,
    pub(super) tracking_leave: bool,
    /// When the window opened: animations are measured from here.
    pub(super) opened: Instant,
    /// The timer that asks for pictures while something moves is running.
    pub(super) timer: bool,
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

    pub(super) fn handler(&mut self) -> &mut H {
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
            // SAFETY: paints at once what was invalidated (so the picture that says
            // "reading" is on screen before the wait), then posts a message to a
            // live window.
            unsafe {
                let _ = UpdateWindow(hwnd);
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
        let now = self.opened.elapsed();
        let list = self
            .handler()
            .frame(w as f32 / scale, h as f32 / scale, renderer.text(), now);
        match renderer.draw(&list) {
            Ok(Drawn::Done) => {
                self.renderer = Some(renderer);
                self.pace(hwnd);
            }
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

    /// Runs one tick of the program. If it has more to do (a background job still
    /// going), the next one is asked for with a timer and not by posting a message:
    /// posted messages go before the mouse and the painting, so posting one after
    /// another would leave the window unable to answer; a timer message goes after.
    fn run_tick(&mut self, hwnd: HWND) {
        let reaction = self.handler().tick();
        self.react(
            hwnd,
            Reaction {
                tick: false,
                ..reaction
            },
        );
        if reaction.tick {
            // SAFETY: sets a timer on a live window.
            unsafe {
                SetTimer(Some(hwnd), TICK_TIMER, TICK_EVERY, None);
            }
        }
    }

    /// While something moves, a timer asks for a picture about every 16 ms; when
    /// nothing moves the timer stops and the window waits for messages again.
    fn pace(&mut self, hwnd: HWND) {
        let moving = self.handler().animating();
        // SAFETY: starts or stops the timer of a live window.
        unsafe {
            if moving && !self.timer {
                self.timer = SetTimer(Some(hwnd), FRAME_TIMER, 16, None) != 0;
            } else if !moving && self.timer {
                let _ = KillTimer(Some(hwnd), FRAME_TIMER);
                self.timer = false;
            }
        }
    }

    #[expect(clippy::too_many_lines, reason = "legacy: split")]
    pub(super) fn message(
        &mut self,
        hwnd: HWND,
        msg: u32,
        wp: WPARAM,
        lp: LPARAM,
    ) -> Option<LRESULT> {
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
                // The window keeps the mouse while the button is down, so dragging the
                // scroll bar goes on even when the pointer leaves the window.
                // SAFETY: captures the mouse for a live window.
                unsafe {
                    SetCapture(hwnd);
                }
                let reaction = self.handler().event(event);
                self.react(hwnd, reaction);
                Some(LRESULT(0))
            }
            WM_SETFOCUS => {
                // The user came back to the window: the program may want to read again.
                let reaction = self.handler().event(UiEvent::Focus);
                self.react(hwnd, reaction);
                None
            }
            WM_LBUTTONUP => {
                // SAFETY: gives the mouse back (it was captured on the press).
                unsafe {
                    let _ = ReleaseCapture();
                }
                let reaction = self.handler().event(UiEvent::Release);
                self.react(hwnd, reaction);
                Some(LRESULT(0))
            }
            WM_KEYDOWN => {
                // The keys that scroll; the letters come as `WM_CHAR`.
                let key = match wp.0 as u32 {
                    0x26 => Some(Key::Up),
                    0x28 => Some(Key::Down),
                    0x21 => Some(Key::PageUp),
                    0x22 => Some(Key::PageDown),
                    0x24 => Some(Key::Home),
                    0x23 => Some(Key::End),
                    _ => None,
                };
                match key {
                    Some(key) => {
                        let reaction = self.handler().event(UiEvent::Key(key));
                        self.react(hwnd, reaction);
                        Some(LRESULT(0))
                    }
                    None => None,
                }
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
            WM_TIMER if wp.0 == FRAME_TIMER => {
                self.react(
                    hwnd,
                    Reaction {
                        redraw: true,
                        tick: false,
                    },
                );
                Some(LRESULT(0))
            }
            WM_APP => {
                self.run_tick(hwnd);
                Some(LRESULT(0))
            }
            WM_TIMER if wp.0 == TICK_TIMER => {
                // SAFETY: stops the timer of a live window; it is set again if needed.
                unsafe {
                    let _ = KillTimer(Some(hwnd), TICK_TIMER);
                }
                self.run_tick(hwnd);
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
