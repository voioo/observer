use log::debug;
use std::fs;

pub struct CPUTopology {
    pub physical_cores: Vec<(usize, usize)>, // (core_id, ht_sibling_id)
}

impl Default for CPUTopology {
    fn default() -> Self {
        Self::new()
    }
}

impl CPUTopology {
    pub fn new() -> Self {
        let mut pairs = Vec::new();
        let cpu_path = "/sys/devices/system/cpu";

        for core_id in 0.. {
            let topology_path =
                format!("{}/cpu{}/topology/thread_siblings_list", cpu_path, core_id);

            if fs::metadata(&topology_path).is_err() {
                break;
            }

            if let Ok(siblings) = fs::read_to_string(&topology_path) {
                let nums: Vec<usize> = siblings
                    .trim()
                    .split(',')
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if nums.len() == 2 && nums[0] == core_id {
                    pairs.push((nums[0], nums[1]));
                }
            }
        }

        debug!("Found CPU pairs (physical, HT): {:?}", pairs);
        CPUTopology {
            physical_cores: pairs,
        }
    }

    pub fn get_cores_to_enable(&self, target_count: usize) -> Vec<usize> {
        let mut cores = Vec::new();

        // Always enable CPU 0 and its sibling if exists
        cores.push(0);
        if let Some((_, sibling)) = self.physical_cores.iter().find(|(core, _)| *core == 0) {
            cores.push(*sibling);
        }

        // Enable additional cores in pairs
        for &(core, sibling) in self.physical_cores.iter().skip(1) {
            if cores.len() >= target_count {
                break;
            }
            cores.push(core);
            cores.push(sibling);
        }

        cores.sort();
        debug!("Enabling cores: {:?}", cores);
        cores
    }
}
