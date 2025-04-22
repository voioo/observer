pub mod power;

// Remove unused direct exports
// pub use cpu::{read_cpu_online_state, set_cpu_online_state};
// pub use power::{get_power_state, PowerState};

// Keep PowerState export if it's used externally (e.g., by CoreManager or main)
// If PowerState is only used within the system module, this can be removed too.
// For now, let's assume it might be needed elsewhere.
pub use power::PowerState;
