use log::{debug, error, info, warn};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use observer::{
    config::{self, Settings},
    core::CoreManager,
    system::power::is_on_battery,
    utils::logging,
};

fn main() -> Result<(), Box<dyn Error>> {
    logging::init();
    println!("Starting Observer...");
    info!("Starting Observer");

    let available_cores = CoreManager::get_available_cores();
    println!(
        "Found {} CPU cores: {:?}",
        available_cores.len(),
        available_cores
    );

    let settings = match config::load_config() {
        Ok(settings) => {
            info!("Loaded configuration: {:?}", settings.clone());
            settings
        }
        Err(e) => {
            warn!("Failed to load config, using defaults: {}", e);
            Settings::default()
        }
    };

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        info!("Shutdown signal received, initiating cleanup...");
        r.store(false, Ordering::SeqCst);
    })?;

    let check_interval = settings.check_interval_sec;
    let mut core_manager = CoreManager::new(settings);
    let power_supply_path = "/sys/class/power_supply/";

    info!("Starting main service loop");
    while running.load(Ordering::SeqCst) {
        match is_on_battery(power_supply_path) {
            Ok(on_battery) => {
                debug!(
                    "Power state check - running on {}",
                    if on_battery { "battery" } else { "AC power" }
                );
                let optimal_cores = core_manager.get_optimal_core_count(on_battery);
                debug!(
                    "Calculated optimal cores: {} (current: {})",
                    optimal_cores,
                    core_manager.current_cores()
                );

                if let Err(e) = core_manager.manage_cpu_cores(optimal_cores) {
                    error!("Failed to manage CPU cores: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to determine power state: {}", e);
            }
        }

        debug!("Sleeping for {} seconds", check_interval);
        thread::sleep(Duration::from_secs(check_interval));
    }

    info!("Service shutting down");
    Ok(())
}
