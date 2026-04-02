use std::io::{Read, Write};
use std::mem;
use std::ptr;
use std::thread;

use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

use tokio::sync::mpsc;
use x11_dl::xinput2;
use x11_dl::xlib;

use crate::error::Error;
use crate::events::Event;
use crate::output;

const XI2_MAJOR_VERSION: i32 = 2;
const XI2_MINOR_VERSION: i32 = 0;

pub struct PhysicalInputMonitor {
    stop_tx: Option<UnixStream>,
    worker: Option<thread::JoinHandle<()>>,
}

impl PhysicalInputMonitor {
    pub fn new(event_tx: mpsc::Sender<Event>) -> Result<Self, Error> {
        let (stop_rx, stop_tx) = UnixStream::pair()
            .map_err(|err| Error::X11(format!("failed to create stop pipe: {}", err)))?;
        let worker = thread::spawn(move || {
            if let Err(err) = monitor_physical_input(event_tx, stop_rx) {
                output::error(format!("Physical input monitor failed: {}", err));
            }
        });

        Ok(Self {
            stop_tx: Some(stop_tx),
            worker: Some(worker),
        })
    }
}

fn monitor_physical_input(
    event_tx: mpsc::Sender<Event>,
    mut stop_rx: UnixStream,
) -> Result<(), Error> {
    let xlib =
        xlib::Xlib::open().map_err(|err| Error::X11(format!("failed to load Xlib: {}", err)))?;
    let xi2 = xinput2::XInput2::open()
        .map_err(|err| Error::X11(format!("failed to load XInput2: {}", err)))?;

    let display = unsafe { (xlib.XOpenDisplay)(ptr::null()) };
    if display.is_null() {
        return Err(Error::X11("failed to open X display".into()));
    }

    let result = monitor_loop(&xlib, &xi2, display, event_tx, &mut stop_rx);

    unsafe {
        (xlib.XCloseDisplay)(display);
    }

    result
}

fn monitor_loop(
    xlib: &xlib::Xlib,
    xi2: &xinput2::XInput2,
    display: *mut xlib::Display,
    event_tx: mpsc::Sender<Event>,
    stop_rx: &mut UnixStream,
) -> Result<(), Error> {
    let root = unsafe { (xlib.XDefaultRootWindow)(display) };
    let mut xi_opcode = 0;
    let mut first_event = 0;
    let mut first_error = 0;
    let extension_name = b"XInputExtension\0";
    let extension_available = unsafe {
        (xlib.XQueryExtension)(
            display,
            extension_name.as_ptr().cast(),
            &mut xi_opcode,
            &mut first_event,
            &mut first_error,
        )
    };

    if extension_available == 0 {
        return Err(Error::X11("XInput extension is not available".into()));
    }

    let mut major = XI2_MAJOR_VERSION;
    let mut minor = XI2_MINOR_VERSION;
    let version_status = unsafe { (xi2.XIQueryVersion)(display, &mut major, &mut minor) };
    if version_status != xlib::Success as i32 {
        return Err(Error::X11(format!(
            "XInput2 version negotiation failed with status {}",
            version_status
        )));
    }

    let mut mask = vec![0; ((xinput2::XI_LASTEVENT + 7) / 8) as usize];
    xinput2::XISetMask(&mut mask, xinput2::XI_RawKeyPress);
    xinput2::XISetMask(&mut mask, xinput2::XI_RawButtonPress);
    xinput2::XISetMask(&mut mask, xinput2::XI_RawMotion);

    let mut event_mask = xinput2::XIEventMask {
        deviceid: xinput2::XIAllDevices,
        mask_len: mask.len() as i32,
        mask: mask.as_mut_ptr(),
    };

    let select_status = unsafe { (xi2.XISelectEvents)(display, root, &mut event_mask, 1) };
    if select_status != xlib::Success as i32 {
        return Err(Error::X11("failed to select XInput2 raw events".into()));
    }

    unsafe {
        (xlib.XFlush)(display);
    }

    let x_fd = unsafe { (xlib.XConnectionNumber)(display) };
    let stop_fd = stop_rx.as_raw_fd();

    loop {
        let mut poll_fds = [
            libc::pollfd {
                fd: x_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stop_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let poll_status = unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as _, -1) };
        if poll_status < 0 {
            return Err(Error::X11(format!(
                "failed while waiting for X11 input events: {}",
                std::io::Error::last_os_error()
            )));
        }

        if poll_fds[1].revents & libc::POLLIN != 0 {
            let mut stop_byte = [0_u8; 1];
            let _ = stop_rx.read(&mut stop_byte);
            return Ok(());
        }

        if poll_fds[0].revents & libc::POLLIN == 0 {
            continue;
        }

        while unsafe { (xlib.XPending)(display) } > 0 {
            let mut event = unsafe { mem::zeroed::<xlib::XEvent>() };
            unsafe {
                (xlib.XNextEvent)(display, &mut event);
            }

            if event.get_type() != xlib::GenericEvent {
                continue;
            }

            let cookie: &mut xlib::XGenericEventCookie = event.as_mut();
            if cookie.extension != xi_opcode {
                continue;
            }

            let got_data = unsafe { (xlib.XGetEventData)(display, cookie) };
            if got_data == 0 {
                return Err(Error::X11("failed to read XInput2 event data".into()));
            }

            let is_activity = matches!(
                cookie.evtype,
                xinput2::XI_RawKeyPress | xinput2::XI_RawButtonPress | xinput2::XI_RawMotion
            );

            unsafe {
                (xlib.XFreeEventData)(display, cookie);
            }

            if is_activity {
                let _ = event_tx.blocking_send(Event::PhysicalInput);
                return Ok(());
            }
        }
    }
}

impl Drop for PhysicalInputMonitor {
    fn drop(&mut self) {
        if let Some(mut stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.write_all(&[1]);
        }

        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
