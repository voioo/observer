use log::{debug, warn};
use std::io;

#[cfg(target_os = "linux")]
use std::fs;

#[cfg(target_os = "linux")]
pub fn read_cpu_online_state(cpu_number: usize) -> io::Result<bool> {
    let path = format!("/sys/devices/system/cpu/cpu{}/online", cpu_number);
    if cpu_number == 0 {
        return Ok(true); // CPU0 is always online
    }

    match fs::read_to_string(&path) {
        Ok(content) => {
            let is_online = content.trim() == "1";
            debug!("CPU {} online state: {}", cpu_number, is_online);
            Ok(is_online)
        }
        Err(e) => {
            // Treat NotFound as offline, propagate other errors
            if e.kind() == io::ErrorKind::NotFound {
                debug!("CPU {} sysfs path not found, assuming offline", cpu_number);
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn read_cpu_online_state(cpu_number: usize) -> io::Result<bool> {
    debug!("CPU online state check not supported on non-Linux. Assuming CPU {} is online.", cpu_number);
    Ok(true) // Assume cores are always online if we can't check/control
}

#[cfg(target_os = "linux")]
pub fn set_cpu_online_state(cpu_number: usize, online: bool) -> io::Result<()> {
    if cpu_number == 0 {
        warn!("Attempted to change state of CPU0, which is not allowed.");
        return Ok(()); // Cannot change CPU0 state
    }

    let path = format!("/sys/devices/system/cpu/cpu{}/online", cpu_number);
    debug!("Setting CPU {} online state to: {}", cpu_number, online);
    fs::write(&path, if online { "1" } else { "0" })
}

#[cfg(not(target_os = "linux"))]
pub fn set_cpu_online_state(cpu_number: usize, online: bool) -> io::Result<()> {
    if cpu_number == 0 {
        return Ok(()); // Still respect the CPU0 rule
    }
    warn!(
        "CPU online state control is not supported on non-Linux. Ignoring request for CPU {}: {}",
        cpu_number, online
    );
    Ok(()) // Do nothing on non-Linux
}
