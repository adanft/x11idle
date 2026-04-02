use std::os::fd::AsRawFd;

use crate::error::Error;
use crate::logind;
use crate::output;

pub struct SleepInhibit {
    fd: Option<zbus::zvariant::OwnedFd>,
}

impl SleepInhibit {
    pub async fn new(conn: &zbus::Connection) -> Result<Self, Error> {
        let reply = conn
            .call_method(
                Some(logind::SERVICE),
                logind::PATH,
                Some(logind::MANAGER_IFACE),
                "Inhibit",
                &("sleep", "x11idle", "x11idle is handling before_sleep_cmd", "delay"),
            )
            .await
            .map_err(|err| Error::Inhibit(format!("failed to request sleep inhibition: {}", err)))?;

        let fd: zbus::zvariant::OwnedFd = reply
            .body()
            .deserialize()
            .map_err(|err| Error::Inhibit(format!("failed to read sleep inhibitor fd: {}", err)))?;
        output::debug(format!("Sleep inhibited (fd: {})", fd.as_raw_fd()));

        Ok(Self { fd: Some(fd) })
    }

    pub fn release(&mut self) {
        if self.fd.take().is_some() {
            output::debug("Releasing sleep inhibitor");
        }
    }
}
