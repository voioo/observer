use log::debug;
use std::error::Error;
use std::fs;

pub fn is_on_battery(power_path: &str) -> Result<bool, Box<dyn Error>> {
    let entries = fs::read_dir(power_path)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.to_string_lossy().contains("AC") {
            let online_path = path.join("online");
            if let Ok(content) = fs::read_to_string(online_path) {
                let on_battery = content.trim() == "0";
                debug!("Power state: {}", if on_battery { "battery" } else { "AC" });
                return Ok(on_battery);
            }
        }
    }

    Ok(false)
}
