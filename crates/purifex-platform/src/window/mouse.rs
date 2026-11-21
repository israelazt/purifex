//! What the pointer does to the window: moving, leaving, the buttons and the wheel, each one
//! turned into a `UiEvent` for the program.

use purifex_domain::ui_event::{UiEvent, UiHandler};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};

use super::state::{high_word, low_word, State};

/// Units scrolled by one notch of the wheel.
const WHEEL_NOTCH: f32 = 64.0;

impl<H: UiHandler> State<H> {
    pub(super) fn on_mouse_move(&mut self, hwnd: HWND, lp: LPARAM) -> Option<LRESULT> {
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
        let (x, y) = self.point(lp);
        self.send(hwnd, UiEvent::Move { x, y })
    }

    /// The pointer left the window; the next movement asks to be told again.
    pub(super) fn on_mouse_leave(&mut self, hwnd: HWND) -> Option<LRESULT> {
        self.tracking_leave = false;
        self.send(hwnd, UiEvent::Leave)
    }

    pub(super) fn on_button_down(&mut self, hwnd: HWND, lp: LPARAM) -> Option<LRESULT> {
        // The window keeps the mouse while the button is down, so dragging the
        // scroll bar goes on even when the pointer leaves the window.
        // SAFETY: captures the mouse for a live window.
        unsafe {
            SetCapture(hwnd);
        }
        let (x, y) = self.point(lp);
        self.send(hwnd, UiEvent::Press { x, y })
    }

    pub(super) fn on_button_up(&mut self, hwnd: HWND) -> Option<LRESULT> {
        // SAFETY: gives the mouse back (it was captured on the press).
        unsafe {
            let _ = ReleaseCapture();
        }
        self.send(hwnd, UiEvent::Release)
    }

    /// The position of the pointer, in units, from the `lp` of a mouse message.
    pub(super) fn point(&self, lp: LPARAM) -> (f32, f32) {
        (self.units(low_word(lp.0)), self.units(high_word(lp.0)))
    }

    pub(super) fn on_wheel(&mut self, hwnd: HWND, wp: WPARAM) -> Option<LRESULT> {
        let notches = high_word(wp.0 as isize) as f32 / 120.0;
        self.send(
            hwnd,
            UiEvent::Wheel {
                dy: -notches * WHEEL_NOTCH,
            },
        )
    }

    /// The right button: a context menu where the pointer is.
    pub(super) fn on_secondary_down(&mut self, hwnd: HWND, lp: LPARAM) -> Option<LRESULT> {
        let (x, y) = self.point(lp);
        self.send(hwnd, UiEvent::SecondaryPress { x, y })
    }
}
