use log::debug;
use std::fs;
use std::io;

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
            if e.kind() == io::ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

pub fn set_cpu_online_state(cpu_number: usize, online: bool) -> io::Result<()> {
    if cpu_number == 0 {
        return Ok(()); // Cannot change CPU0 state
    }

    let path = format!("/sys/devices/system/cpu/cpu{}/online", cpu_number);
    fs::write(&path, if online { "1" } else { "0" })
}

pub fn get_cpu_info() -> io::Result<String> {
    fs::read_to_string("/proc/cpuinfo")
}
