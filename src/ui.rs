use crate::about;
use crate::config::{ShiftModifiers};
use crate::device::VpcDevice; // Assuming VpcDevice has Display impl
use crate::{ShiftTool, INITIAL_HEIGHT, INITIAL_WIDTH, PROGRAM_TITLE}; // Import main struct
use crate::state::State;
use crate::util::read_bit; // Import utility
use eframe::egui::{self, Color32, Context, ScrollArea, Ui};

const DISABLED_COLOR: Color32 = Color32::from_rgb(255, 0, 0); // Red for disabled

// Keep UI drawing functions associated with ShiftTool
impl ShiftTool {
    // --- Button/Action Handlers (called from draw_running_state) ---

    fn handle_start_stop_toggle(&mut self) {
        if self.config.data.sources.is_empty()
            || self.config.data.receivers.is_empty()
        {
            log::warn!("Start/Stop ignored: No source or receiver selected.");
            return; // Don't toggle if no devices configured
        }

        let was_started;
        {
            let &(ref lock, ref cvar) = &*self.thread_state;
            let mut started_guard = lock.lock().expect("Thread state mutex poisoned");
            was_started = *started_guard;
            *started_guard = !was_started; // Toggle the state
            log::info!("Toggled worker thread state to: {}", *started_guard);
            cvar.notify_all(); // Notify thread if it was waiting
        } // Mutex guard dropped here

        if !was_started {
            // If we just started it
            if !self.spawn_worker() {
                // If spawning failed, revert the state
                log::error!("Worker thread failed to spawn, reverting state.");
                let &(ref lock, ref cvar) = &*self.thread_state;
                let mut started_guard = lock.lock().expect("Thread state mutex poisoned");
                *started_guard = false;
                cvar.notify_all();
            } else {
                log::info!("Worker thread started.");
                // Save config on start
                if let Err(e) = self.config.save() {
                    log::error!("Failed to save config on start: {}", e);
                }
            }
        } else {
            // If we just stopped it
            log::info!("Worker thread stopped.");
            self.stop_worker_cleanup(); // Perform cleanup actions
            // Save config on stop
            if let Err(e) = self.config.save() {
                log::error!("Failed to save config on stop: {}", e);
            }
        }
    }

    fn handle_add_source(&mut self) {
        self.add_source_state(); // Add state tracking
        self.config.data.sources.push(Default::default()); // Add config entry
        log::debug!("Added source device slot.");
    }

    fn handle_remove_source(&mut self) {
        if self.config.data.sources.len() > 1 {
            self.source_states.pop();
            self.config.data.sources.pop();
            log::debug!("Removed last source device slot.");
        }
    }

    fn handle_add_receiver(&mut self) {
        self.add_receiver_state(); // Add state tracking
        self.config.data.receivers.push(Default::default()); // Add config entry
        log::debug!("Added receiver device slot.");
    }

    fn handle_remove_receiver(&mut self) {
        if !self.config.data.receivers.is_empty() {
            self.receiver_states.pop();
            self.config.data.receivers.pop();
            log::debug!("Removed last receiver device slot.");
        }
    }
}

// --- UI Drawing Functions ---

pub(crate) fn draw_about_screen(app: &mut ShiftTool, ui: &mut Ui) {
    ui.set_width(INITIAL_WIDTH);
    ui.vertical_centered(|ui| {
        ui.heading(format!("About {}", PROGRAM_TITLE));
        ui.separator();
        for line in about::about() {
            ui.label(line);
        }
        ui.separator();
        if ui.button("OK").clicked() {
            app.state = State::Running;
        }
    });
}

pub(crate) fn draw_running_state(
    app: &mut ShiftTool,
    ui: &mut Ui,
    ctx: &Context,
) {
    let thread_running = app.get_thread_status();
    app.refresh_devices(); // Need to be careful about frequent HID API calls

    if app.config.data.sources.is_empty() {
        // Ensure at least one source slot exists initially
        app.handle_add_source();
    }

    ui.columns(2, |columns| {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(&mut columns[0], |ui| {
                ui.vertical(|ui| {
                    draw_sources_section(app, ui, thread_running);
                    ui.separator();
                    draw_rules_section(app, ui, thread_running);
                    ui.separator();
                    draw_receivers_section(app, ui, thread_running);
                    ui.add_space(10.0);
                });
            });

        columns[1].vertical(|ui| {
            draw_control_buttons(app, ui, ctx, thread_running);
        });
    });
}

fn draw_sources_section(
    app: &mut ShiftTool,
    ui: &mut Ui,
    thread_running: bool,
) {
    ui.heading("Sources");
    for i in 0..app.config.data.sources.len() {
        // --- Immutable Operations First ---
        let saved_config_for_find = app.config.data.sources[i].clone();
        let selected_device_idx = crate::device::find_device_index_for_saved(
            &app.device_list, // Pass immutable borrow of device_list
            &saved_config_for_find,
        );

        // --- Now get mutable borrow for UI elements that might change config ---
        let source_config = &mut app.config.data.sources[i];
        let device_list = &app.device_list; // Re-borrow immutably (allowed alongside mutable borrow of a *different* field)
        let source_states = &app.source_states;

        let vid = source_config.vendor_id;
        let pid = source_config.product_id;

        ui.horizontal(|ui| {
            ui.label(format!("Source {}:", i + 1));
            // Device Selector Combo Box
            device_selector_combo(
                ui,
                format!("source_combo_{}", i),
                device_list, // Pass immutable borrow
                selected_device_idx,
                |selected_idx| {
                    if selected_idx < device_list.len() { // Bounds check
                        source_config.vendor_id = device_list[selected_idx].vendor_id;
                        source_config.product_id = device_list[selected_idx].product_id;
                        source_config.serial_number =
                            device_list[selected_idx].serial_number.clone();
                    }
                },
                thread_running,
            );
        }); // Mutable borrow of source_config might end here or after status bits

        // Draw status bits for this source
        if let Some(state_arc) = source_states.get(i) {
            let state_val = match state_arc.lock() { // Use match
                Ok(guard) => {
                    log::debug!("UI: Reading source_states[{}] = {}", i, *guard);
                    *guard // Dereference the guard to get the value
                }
                Err(poisoned) => {
                    log::error!("UI: Mutex poisoned for source_states[{}]!", i);
                    **poisoned.get_ref() // Try to get value anyway
                }
            };

            // Pass mutable borrow of state_enabled part of source_config
            draw_status_bits(
                ui,
                "   Shift:",
                state_val,
                &mut source_config.state_enabled,
                vid,
                pid,
                thread_running,
                thread_running,
                true
            );
        } else {
            ui.colored_label(Color32::RED, "Error: State mismatch");
        }

        ui.add_space(5.0);
    } // Mutable borrow of source_config definitely ends here
    ui.add_space(10.0);
}

fn draw_rules_section(
    app: &mut ShiftTool,
    ui: &mut Ui,
    thread_running: bool,
) {
    ui.heading("Rules & Result");
    ui.horizontal(|ui| {
        ui.label("Rules:");
        ui.add_enabled_ui(!thread_running, |ui| {
            for j in 0..8 {
                let current_modifier = app.config.data.shift_modifiers[j];
                if ui
                    .selectable_label(false, format!("{}", current_modifier))
                    .clicked()
                {
                    // Cycle through modifiers on click
                    app.config.data.shift_modifiers[j] = match current_modifier {
                        ShiftModifiers::OR => ShiftModifiers::AND,
                        ShiftModifiers::AND => ShiftModifiers::XOR,
                        ShiftModifiers::XOR => ShiftModifiers::OR,
                    };
                }
            }
        });
    });

    // Display combined result state
    let final_state_val = *app.shift_state.lock().unwrap();
    draw_status_bits(
        ui,
        "Result:",
        final_state_val,
        &mut [true; 8], // Pass dummy array
        0,
        0,
        false,
        true,
        false,
    );
    ui.add_space(10.0); // Space after the section
}

fn draw_receivers_section(
    app: &mut ShiftTool,
    ui: &mut Ui,
    thread_running: bool,
) {
    ui.heading("Receivers");
    if app.config.data.receivers.is_empty() {
        ui.label("(Add a receiver using the controls on the right)");
    }
    // Iterate by index
    for i in 0..app.config.data.receivers.len() {
        // --- Immutable Operations First ---
        let saved_config_for_find = app.config.data.receivers[i].clone();
        let selected_device_idx = crate::device::find_device_index_for_saved(
            &app.device_list,
            &saved_config_for_find,
        );

        // --- Mutable Borrow Scope ---
        let receiver_config = &mut app.config.data.receivers[i];
        let device_list = &app.device_list;
        let receiver_states = &app.receiver_states;

        let vid = receiver_config.vendor_id;
        let pid = receiver_config.product_id;

        ui.horizontal(|ui| {
            ui.label(format!("Receiver {}:", i + 1));
            device_selector_combo(
                ui,
                format!("receiver_combo_{}", i),
                device_list,
                selected_device_idx,
                |selected_idx| {
                    if selected_idx < device_list.len() { // Bounds check
                        receiver_config.vendor_id = device_list[selected_idx].vendor_id;
                        receiver_config.product_id = device_list[selected_idx].product_id;
                        receiver_config.serial_number =
                            device_list[selected_idx].serial_number.clone();
                    }
                },
                thread_running,
            );
        }); // Mut borrow might end here

        if let Some(state_arc) = receiver_states.get(i) {
            let state_val = match state_arc.lock() { // Use match
                Ok(guard) => {
                    log::debug!("UI: Reading receiver_states[{}] = {}", i, *guard);
                    *guard // Dereference the guard to get the value
                }
                Err(poisoned) => {
                    log::error!("UI: Mutex poisoned for receiver_states[{}]!", i);
                    **poisoned.get_ref() // Try to get value anyway
                }
            };
            draw_status_bits(
                ui,
                "   Shift:",
                state_val,
                &mut receiver_config.state_enabled, // Pass mut borrow
                vid,
                pid,
                thread_running,
                thread_running,
                true
            );
        } else {
            ui.colored_label(Color32::RED, "Error: State mismatch");
        }

        ui.add_space(5.0);
    } // Mut borrow ends here
}

// --- UI Helper Widgets ---

/// Creates a ComboBox for selecting a device.
fn device_selector_combo(
    ui: &mut Ui,
    id_source: impl std::hash::Hash,
    device_list: &[VpcDevice],
    selected_device_idx: usize,
    mut on_select: impl FnMut(usize), // Closure called when selection changes
    disabled: bool,
) {
    let selected_text = if selected_device_idx < device_list.len() {
        format!("{}", device_list[selected_device_idx])
    } else {
        // Handle case where index might be out of bounds after a refresh
        "-SELECT DEVICE-".to_string()
    };

    ui.add_enabled_ui(!disabled, |ui| {
        egui::ComboBox::from_id_source(id_source)
            .width(300.0) // Adjust width as needed
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for (j, device) in device_list.iter().enumerate() {
                    // Use selectable_value to handle selection logic
                    if ui
                        .selectable_label(
                            j == selected_device_idx,
                            format!("{}", device),
                        )
                        .clicked()
                    {
                        if j != selected_device_idx {
                            on_select(j); // Call the provided closure
                        }
                    }
                }
            });
    });
}

/// Draws the row of shift status bits (1-5, DTNT, ZOOM, TRIM).
fn draw_status_bits(
    ui: &mut Ui,
    label: &str,
    state_value: u16,
    enabled_mask: &mut [bool; 8],
    vendor_id: u16,
    product_id: u16,
    thread_running: bool,
    bits_disabled: bool, // If the whole row should be unclickable
    show_online_status: bool,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        log::debug!("draw_status_bits received state_value: {}", state_value);

        ui.add_enabled_ui(!bits_disabled, |ui| {
            // Bits 0-4 (Shift 1-5)
            for j in 0..5u8 {
                let bit_is_set = read_bit(state_value, j);
                let is_enabled = enabled_mask[j as usize];
                let color = if !is_enabled {
                    DISABLED_COLOR
                } else {
                    Color32::TRANSPARENT // Default background
                };

                log::debug!(
                    "  Bit {}: state={}, enabled={}, calculated_selected={}",
                    j, state_value, is_enabled, bit_is_set
                );

                // Use selectable_value for clickable behavior
                if ui
                    .selectable_label(
                        bit_is_set,
                        egui::RichText::new(format!("{}", j + 1))
                            .background_color(color),
                    )
                    .clicked()
                {
                    // Toggle the enabled state if clicked
                    enabled_mask[j as usize] = !is_enabled;
                }
            }

            // Special Bits (DTNT, ZOOM, TRIM) - Assuming order 5, 6, 7
            let special_bits = [("DTNT", 5u8), ("ZOOM", 6u8), ("TRIM", 7u8)];
            for (name, bit_pos) in special_bits {
                let bit_is_set = read_bit(state_value, bit_pos);
                let is_enabled = enabled_mask[bit_pos as usize];
                let color = if !is_enabled {
                    DISABLED_COLOR
                } else {
                    Color32::TRANSPARENT
                };
                log::debug!(
                    "  Bit {}: name={}, state={}, enabled={}, calculated_selected={}",
                    bit_pos, name, state_value, is_enabled, bit_is_set
                );

                if ui
                    .selectable_label(
                        bit_is_set,
                        egui::RichText::new(name).background_color(color),
                    )
                    .clicked()
                {
                    enabled_mask[bit_pos as usize] = !is_enabled;
                }
            }
        });

        // --- Draw the Online/Offline Status ---
        // Add some spacing before the status
        if show_online_status {
            ui.add_space(15.0); // Adjust as needed

            let is_configured = vendor_id != 0 && product_id != 0;
            let (text, color) = if thread_running && is_configured {
                ("ONLINE", Color32::GREEN)
            } else if !is_configured {
                ("UNCONFIGURED", Color32::YELLOW)
            } else {
                ("OFFLINE", Color32::GRAY)
            };
            // Add the status label directly here
            ui.label(egui::RichText::new(text).color(color));
        }
    });
}

/// Draws the ONLINE/OFFLINE status indicator.
fn draw_online_status(
    ui: &mut Ui,
    saved_device_config: &crate::device::SavedDevice, // Pass the config for this slot
    thread_running: bool,
) {
    // Infer status: Online if thread is running AND device is configured (VID/PID != 0)
    let is_configured = saved_device_config.vendor_id != 0 && saved_device_config.product_id != 0;

    // Determine status text and color
    let (text, color) = if thread_running && is_configured {
        // We assume the worker *tries* to talk to configured devices.
        // A more advanced check could involve reading another shared state
        // updated by the worker indicating recent success/failure for this device.
        ("ONLINE", Color32::GREEN)
    } else if !is_configured {
        ("UNCONFIGURED", Color32::YELLOW) // Show if slot is empty
    } else { // Thread not running or device not configured
        ("OFFLINE", Color32::GRAY)
    };

    // Use selectable_label for consistent look, but make it non-interactive
    // Set 'selected' argument to false as it's just a status display
    ui.selectable_label(false, egui::RichText::new(text).color(color));
}



/// Draws the control buttons in the right column.
fn draw_control_buttons(
    app: &mut ShiftTool,
    ui: &mut Ui,
    ctx: &Context,
    thread_running: bool,
) {
    // Start/Stop Button
    let (start_stop_text, start_stop_color) = if thread_running {
        ("Stop", DISABLED_COLOR)
    } else {
        ("Start", Color32::GREEN) // Use Green for Start
    };
    if ui
        .button(
            egui::RichText::new(start_stop_text)
                .color(Color32::BLACK) // Text color
                .background_color(start_stop_color),
        )
        .clicked()
    {
        app.handle_start_stop_toggle();
    }

    // ui.separator();

    // Add/Remove Source Buttons
    if ui.add_enabled(!thread_running, egui::Button::new("Add Source")).clicked() {
        app.handle_add_source();
    }
    if app.config.data.sources.len() > 1 { // Only show remove if more than 1
        if ui.add_enabled(!thread_running, egui::Button::new("Remove Source")).clicked() {
            app.handle_remove_source();
        }
    }

    // ui.separator();

    // Add/Remove Receiver Buttons
    if ui.add_enabled(!thread_running, egui::Button::new("Add Receiver")).clicked() {
        app.handle_add_receiver();
    }
    if !app.config.data.receivers.is_empty() { // Only show remove if > 0
        if ui.add_enabled(!thread_running, egui::Button::new("Remove Receiver")).clicked() {
            app.handle_remove_receiver();
        }
    }

    // ui.separator();

    // Other Buttons
    // if ui.add_enabled(!thread_running, egui::Button::new("Save Config")).clicked() {
    //     if let Err(e) = app.config.save() {
    //         log::error!("Failed to manually save config: {}", e);
    //         // Optionally show feedback to user in UI
    //     } else {
    //         log::info!("Configuration saved manually.");
    //     }
    // }

    if ui.add_enabled(!thread_running, egui::Button::new("Refresh Devices")).clicked() {
        log::info!("Refreshing device list manually.");
        app.refresh_devices();
    }

    if ui.button("About").clicked() {
        app.state = State::About;
    }

    if ui.button("Exit").clicked() {
        // Ask eframe to close the window. `on_exit` will be called.
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
