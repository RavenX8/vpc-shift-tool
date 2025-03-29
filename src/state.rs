// Represents the current high-level state of the application UI
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum State {
    Initialising, // App is starting, loading config, doing initial scan
    Running,      // Main operational state, showing devices and controls
    About,        // Showing the about screen
}
