use x11_dl::xlib;
use x11_dl::xss;

use crate::error::Error;

/// X11 idle time reader. Contains raw X11 pointers, making it `!Send`.
/// App must run on the main thread via `block_on` — do NOT use `tokio::spawn`.
pub struct IdleClock {
    xlib: xlib::Xlib,
    xss: xss::Xss,
    display: *mut xlib::Display,
    root: xlib::Window,
    info: *mut xss::XScreenSaverInfo,
}

impl IdleClock {
    pub fn new() -> Result<Self, Error> {
        let xlib = xlib::Xlib::open()
            .map_err(|err| Error::X11(format!("failed to load Xlib: {}", err)))?;
        let xss = xss::Xss::open()
            .map_err(|err| Error::X11(format!("failed to load XScreenSaver: {}", err)))?;

        let display = unsafe { (xlib.XOpenDisplay)(std::ptr::null()) };
        if display.is_null() {
            return Err(Error::X11("failed to open X display".into()));
        }

        let root = unsafe { (xlib.XDefaultRootWindow)(display) };
        let info = unsafe { (xss.XScreenSaverAllocInfo)() };
        if info.is_null() {
            unsafe {
                (xlib.XCloseDisplay)(display);
            }
            return Err(Error::X11("failed to allocate XScreenSaver info".into()));
        }

        Ok(Self {
            xlib,
            xss,
            display,
            root,
            info,
        })
    }

    pub fn idle_ms(&self) -> Result<u64, Error> {
        let status =
            unsafe { (self.xss.XScreenSaverQueryInfo)(self.display, self.root, self.info) };
        if status == 0 {
            return Err(Error::X11("XScreenSaverQueryInfo failed".into()));
        }

        let idle = unsafe { (*self.info).idle };
        Ok(idle as u64)
    }
}

impl Drop for IdleClock {
    fn drop(&mut self) {
        unsafe {
            if !self.info.is_null() {
                (self.xlib.XFree)(self.info.cast());
            }

            if !self.display.is_null() {
                (self.xlib.XCloseDisplay)(self.display);
            }
        }
    }
}
