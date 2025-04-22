use log::{debug, warn};
use std::error::Error;

#[cfg(target_os = "linux")]
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    AC,
    Battery,
    Unknown,
}

#[cfg(target_os = "linux")]
pub fn get_power_state(power_path: &str) -> Result<PowerState, Box<dyn Error>> {
    let entries = fs::read_dir(power_path)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.to_string_lossy().contains("AC") {
            let online_path = path.join("online");
            match fs::read_to_string(&online_path) {
                Ok(content) => {
                    let state = if content.trim() == "1" {
                        PowerState::AC
                    } else {
                        PowerState::Battery
                    };
                    debug!(
                        "Detected power state from {}: {:?}",
                        online_path.display(),
                        state
                    );
                    return Ok(state); // Return the first definite state found
                }
                Err(_) => {
                    warn!(
                        "Could not read {}: {}",
                        online_path.display(),
                        "status cannot be read"
                    );
                    return Ok(PowerState::Unknown);
                }
            }
        }
    }

    warn!(
        "No AC power supply found or readable in {}. Assuming unknown.",
        power_path
    );
    Ok(PowerState::Unknown) // No AC adapter found or readable
}

#[cfg(not(target_os = "linux"))]
pub fn get_power_state(_power_path: &str) -> Result<PowerState, Box<dyn Error>> {
    warn!("Power status detection is only supported on Linux. Assuming Unknown power state.");
    Ok(PowerState::Unknown)
}
