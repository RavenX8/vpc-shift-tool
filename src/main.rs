#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// Declare modules
mod about;
mod config;
mod device;
mod hid_worker;
mod state;
mod ui;
mod util;

use std::process::exit;
// External Crate Imports (only those needed directly in main.rs)
use eframe::{egui, glow};
use fast_config::Config;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use clap::Parser;

// Internal Module Imports
use config::{ConfigData}; // Import specific items
use device::{VpcDevice, SavedDevice};
use state::State; // Import the State enum

// Constants
const PROGRAM_TITLE: &str = "OpenVPC - Shift Tool";
const INITIAL_WIDTH: f32 = 740.0;
const INITIAL_HEIGHT: f32 = 260.0;

// Type aliases for shared state can make signatures cleaner
pub type SharedStateFlag = Arc<(Mutex<bool>, Condvar)>;
pub type SharedDeviceState = Arc<Mutex<u16>>; // Assuming Condvar isn't strictly needed here

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = false)]
    skip_firmware: bool,
}

// The main application struct
pub struct ShiftTool {
    // State
    state: State,
    thread_state: SharedStateFlag, // Is the worker thread running?

    // Device Data
    device_list: Vec<VpcDevice>, // List of discovered compatible devices

    // Shared state between UI and Worker Thread
    source_states: Vec<SharedDeviceState>, // Current reported state per source
    receiver_states: Vec<SharedDeviceState>, // Current reported state per receiver
    shift_state: SharedDeviceState, // Combined/calculated shift state

    // Configuration
    config: Config<ConfigData>,
}

impl Default for ShiftTool {
    fn default() -> Self {
        // Determine config path safely
        let config_dir = dirs::config_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string()); // Fallback to current dir
        let config_path = format!("{}/shift_tool.json", config_dir);

        // Handle potential config creation error
        let config = match Config::new(&config_path, ConfigData::default()) {
            Ok(cfg) => cfg,
            Err(e) => {
                // Log the error appropriately
                eprintln!("Error creating config file at {}: {}", config_path, e);
                // Fallback to default in-memory config? Or panic?
                // Using default() here might lead to data loss if file exists but is broken.
                // For simplicity here, we proceed with default, but real app might need better handling.
                exit(1)
            }
        };

        Self {
            state: State::Initialising,
            device_list: vec![],
            source_states: vec![],
            receiver_states: vec![],
            shift_state: Arc::new(Mutex::new(0)), // Keep Condvar if needed for shift_state?
            thread_state: Arc::new((Mutex::new(false), Condvar::new())),
            config,
        }
    }
}

// Implementations specific to App lifecycle and top-level control
impl ShiftTool {
    // Initialization logic called once at the start
    fn init(&mut self) {
        // Load config and populate initial sources/receivers based on config
        // The config is already loaded in Default::default()
        let num_sources = self.config.data.sources.len();
        let num_receivers = self.config.data.receivers.len();

        for _ in 0..num_sources {
            self.add_source_state(); // Add state tracking
        }
        for _ in 0..num_receivers {
            self.add_receiver_state(); // Add state tracking
        }

        // Initial device scan
        self.refresh_devices(); // Now calls the method defined in device.rs

        self.state = State::Running;
        log::info!("Initialization complete. State set to Running.");
    }

    // Helper to add state tracking for a new source
    fn add_source_state(&mut self) {
        self.source_states
            .push(Arc::new(Mutex::new(0)));
    }

    // Helper to add state tracking for a new receiver
    fn add_receiver_state(&mut self) {
        self.receiver_states
            .push(Arc::new(Mutex::new(0)));
    }

    // Helper to get thread status (could be in ui.rs or main.rs)
    fn get_thread_status(&self) -> bool {
        match self.thread_state.0.lock() {
            Ok(guard) => *guard,
            Err(poisoned) => {
                log::error!("Thread state mutex poisoned!");
                **poisoned.get_ref() // Still try to get the value
            }
        }
    }

    // Graceful shutdown logic
    fn shutdown_app(&mut self) {
        log::info!("Shutdown requested.");
        // Signal the worker thread to stop
        {
            let &(ref lock, ref cvar) = &*self.thread_state;
            match lock.lock() {
                Ok(mut started) => {
                    *started = false;
                    log::info!("Signaling worker thread to stop.");
                }
                Err(_) => {
                    log::error!("Thread state mutex poisoned during shutdown!");
                }
            }
            cvar.notify_all(); // Wake up thread if it's waiting
        }

        // Save configuration
        if let Err(e) = self.config.save() {
            log::error!("Failed to save configuration on exit: {}", e);
        } else {
            log::info!("Configuration saved.");
        }

        // Give the thread a moment to process the stop signal (optional)
        // Note: Joining the thread handle would be more robust if we kept it.
        std::thread::sleep(Duration::from_millis(250));
        log::info!("Shutdown complete.");
    }
}

// Main eframe application loop
impl eframe::App for ShiftTool {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        log::debug!("Update Called.");
        // Request repaint ensures GUI updates even if worker is slow
        ctx.request_repaint_after(Duration::from_millis(50));

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Resize::default()
                .default_width(INITIAL_WIDTH)
                .default_height(INITIAL_HEIGHT)
                .auto_sized()
                .show(ui, |ui| match self.state {
                    State::Initialising => {
                        // Show a simple "Loading..." message while init runs
                        ui.centered_and_justified(|ui| {
                            ui.label("Initialising...");
                        });
                        // Actual init logic runs once after this frame
                        self.init();
                    }
                    State::About => {
                        // Call the UI drawing function from the ui module
                        ui::draw_about_screen(self, ui);
                    }
                    State::Running => {
                        // Call the UI drawing function from the ui module
                        ui::draw_running_state(self, ui, ctx);
                    }
                });
        });
    }

    // Called when the application is about to close
    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        self.shutdown_app();
    }
}

// Application Entry Point
fn main() -> eframe::Result<()> {
    // Initialize logging
    env_logger::init();

    // --- Command Line Argument Parsing ---
    // let _args = Args::parse();
    // --- End Argument Parsing ---

    log::info!("Starting {}", PROGRAM_TITLE);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([INITIAL_WIDTH, INITIAL_HEIGHT])
            .with_title(PROGRAM_TITLE), // Set window title here
        ..Default::default()
    };

    eframe::run_native(
        PROGRAM_TITLE, // Used for window title if not set in viewport
        options,
        Box::new(|_cc| Ok(Box::new(ShiftTool::default()))), // Create the app instance
    )
}
