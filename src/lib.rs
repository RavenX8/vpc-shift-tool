// Export modules for testing
pub mod about;
pub mod config;
pub mod device;
pub mod hid_worker;
pub mod state;
pub mod ui;
pub mod util;

// Re-export main struct and types for testing
pub use crate::config::ConfigData;
pub use crate::device::VpcDevice;
pub use crate::state::State;

// Constants
pub const PROGRAM_TITLE: &str = "OpenVPC - Shift Tool";
pub const INITIAL_WIDTH: f32 = 740.0;
pub const INITIAL_HEIGHT: f32 = 260.0;

// Type aliases for shared state
pub use std::sync::{Arc, Condvar, Mutex};
pub type SharedStateFlag = Arc<(Mutex<bool>, Condvar)>;
pub type SharedDeviceState = Arc<Mutex<u16>>;

// Args struct for command line parsing
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value_t = false)]
    pub skip_firmware: bool,
}

// Wrapper for ConfigData to match the actual structure
pub use fast_config::Config;

// The main application struct
pub struct ShiftTool {
    // State
    pub state: State,
    pub thread_state: SharedStateFlag, // Is the worker thread running?

    // Device Data
    pub device_list: Vec<VpcDevice>, // List of discovered compatible devices

    // Shared state between UI and Worker Thread
    pub shift_state: SharedDeviceState, // Current shift state
    pub source_states: Vec<SharedDeviceState>, // Current state of each source device
    pub receiver_states: Vec<SharedDeviceState>, // Current state of each receiver device

    // Configuration
    pub config: Config<ConfigData>,
    pub selected_source: usize,
    pub selected_receiver: usize,
}

// Implementations for ShiftTool
impl ShiftTool {
    // Add a new source state tracking object
    pub fn add_source_state(&mut self) {
        self.source_states.push(Arc::new(Mutex::new(0)));
    }

    // Add a new receiver state tracking object
    pub fn add_receiver_state(&mut self) {
        self.receiver_states.push(Arc::new(Mutex::new(0)));
    }

    // Get the current thread status
    pub fn get_thread_status(&self) -> bool {
        let &(ref lock, _) = &*self.thread_state;
        match lock.lock() {
            Ok(guard) => *guard,
            Err(_) => false, // Return false if the mutex is poisoned
        }
    }
}
