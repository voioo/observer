pub mod cpu;
pub mod power;

pub use cpu::{read_cpu_online_state, set_cpu_online_state};
pub use power::is_on_battery;
