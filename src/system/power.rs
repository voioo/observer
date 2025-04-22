use log::warn;
use std::error::Error;

#[cfg(target_os = "linux")]
use std::fs;

#[cfg(target_os = "linux")]
pub fn is_on_battery(power_path: &str) -> Result<bool, Box<dyn Error>> {
    let entries = fs::read_dir(power_path)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.to_string_lossy().contains("AC") {
            let online_path = path.join("online");
            if let Ok(content) = fs::read_to_string(online_path) {
                let on_battery = content.trim() == "0";
                warn!("Linux Power state: {}", if on_battery { "battery" } else { "AC" });
                return Ok(on_battery);
            }
        }
    }

    warn!("Could not determine power status from /sysfs, assuming AC power.");
    Ok(false)
}

#[cfg(not(target_os = "linux"))]
pub fn is_on_battery(_power_path: &str) -> Result<bool, Box<dyn Error>> {
    warn!("Power status detection is only supported on Linux. Assuming AC power.");
    Ok(false)
}
