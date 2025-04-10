use vpc_shift_tool::config::{ConfigData, ShiftModifiers, ModifiersArray};
use vpc_shift_tool::device::{SavedDevice, VpcDevice};
use vpc_shift_tool::state::State;
use std::rc::Rc;

#[test]
fn test_config_data_default() {
    // Test that the default ConfigData is created correctly
    let config = ConfigData::default();

    // Check that sources and receivers are empty
    assert_eq!(config.sources.len(), 0);
    assert_eq!(config.receivers.len(), 0);

    // Check that shift_modifiers has the default value (all OR)
    for i in 0..8 {
        assert_eq!(config.shift_modifiers[i], ShiftModifiers::OR);
    }
}

#[test]
fn test_shift_modifiers_display() {
    // Test the Display implementation for ShiftModifiers
    assert_eq!(format!("{}", ShiftModifiers::OR), "OR");
    assert_eq!(format!("{}", ShiftModifiers::AND), "AND");
    assert_eq!(format!("{}", ShiftModifiers::XOR), "XOR");
}

#[test]
fn test_saved_device_default() {
    // Test that the default SavedDevice is created correctly
    let device = SavedDevice::default();

    assert_eq!(device.vendor_id, 0);
    assert_eq!(device.product_id, 0);
    assert_eq!(device.serial_number, "");
    assert_eq!(device.state_enabled, [true; 8]); // All bits enabled by default
}

#[test]
fn test_state_enum() {
    // Test that the State enum has the expected variants
    let initializing = State::Initialising;
    let about = State::About;
    let running = State::Running;

    // Test that the variants are different
    assert_ne!(initializing, about);
    assert_ne!(initializing, running);
    assert_ne!(about, running);

    // Test equality with same variant
    assert_eq!(initializing, State::Initialising);
    assert_eq!(about, State::About);
    assert_eq!(running, State::Running);
}

#[test]
fn test_config_with_devices() {
    // Test creating a ConfigData with sources and receivers
    let mut config = ConfigData::default();

    // Create some test devices
    let device1 = SavedDevice {
        vendor_id: 0x3344,
        product_id: 0x0001,
        serial_number: "123456".to_string(),
        state_enabled: [true, false, true, false, true, false, true, false],
    };

    let device2 = SavedDevice {
        vendor_id: 0x3344,
        product_id: 0x0002,
        serial_number: "654321".to_string(),
        state_enabled: [false, true, false, true, false, true, false, true],
    };

    // Add devices to sources and receivers
    config.sources.push(device1.clone());
    config.receivers.push(device2.clone());

    // Check that the devices were added correctly
    assert_eq!(config.sources.len(), 1);
    assert_eq!(config.receivers.len(), 1);

    assert_eq!(config.sources[0].vendor_id, 0x3344);
    assert_eq!(config.sources[0].product_id, 0x0001);
    assert_eq!(config.sources[0].serial_number, "123456");
    assert_eq!(config.sources[0].state_enabled, [true, false, true, false, true, false, true, false]);

    assert_eq!(config.receivers[0].vendor_id, 0x3344);
    assert_eq!(config.receivers[0].product_id, 0x0002);
    assert_eq!(config.receivers[0].serial_number, "654321");
    assert_eq!(config.receivers[0].state_enabled, [false, true, false, true, false, true, false, true]);
}

#[test]
fn test_modifiers_array() {
    // Test the ModifiersArray implementation
    let mut modifiers = ModifiersArray::default();

    // Check default values
    for i in 0..8 {
        assert_eq!(modifiers[i], ShiftModifiers::OR);
    }

    // Test setting values
    modifiers[0] = ShiftModifiers::AND;
    modifiers[4] = ShiftModifiers::XOR;

    // Check the modified values
    assert_eq!(modifiers[0], ShiftModifiers::AND);
    assert_eq!(modifiers[4], ShiftModifiers::XOR);

    // Check that other values remain unchanged
    for i in 1..4 {
        assert_eq!(modifiers[i], ShiftModifiers::OR);
    }
    for i in 5..8 {
        assert_eq!(modifiers[i], ShiftModifiers::OR);
    }
}

#[test]
fn test_vpc_device_default() {
    // Test the default VpcDevice implementation
    let device = VpcDevice::default();

    assert_eq!(device.full_name, "");
    assert_eq!(*device.name, "-NO CONNECTION (Select device from list)-");
    assert_eq!(*device.firmware, "");
    assert_eq!(device.vendor_id, 0);
    assert_eq!(device.product_id, 0);
    assert_eq!(device.serial_number, "");
    assert_eq!(device.usage, 0);
    assert_eq!(device.active, false);
}

#[test]
fn test_vpc_device_display() {
    // Test the Display implementation for VpcDevice

    // Test default device
    let device = VpcDevice::default();
    assert_eq!(format!("{}", device), "-NO CONNECTION (Select device from list)-");

    // Test a real device
    let device = VpcDevice {
        full_name: "3344:0001:123456".to_string(),
        name: Rc::new("VPC MongoosT-50CM3".to_string()),
        firmware: Rc::new("VIRPIL Controls 20240101".to_string()),
        vendor_id: 0x3344,
        product_id: 0x0001,
        serial_number: "123456".to_string(),
        usage: 0,
        active: false,
    };

    assert_eq!(
        format!("{}", device),
        "VID:3344 PID:0001 VPC MongoosT-50CM3 (SN:123456 FW:VIRPIL Controls 20240101)"
    );

    // Test a device with empty serial number
    let device = VpcDevice {
        full_name: "3344:0001:no_sn".to_string(),
        name: Rc::new("VPC MongoosT-50CM3".to_string()),
        firmware: Rc::new("VIRPIL Controls 20240101".to_string()),
        vendor_id: 0x3344,
        product_id: 0x0001,
        serial_number: "".to_string(),
        usage: 0,
        active: false,
    };

    assert_eq!(
        format!("{}", device),
        "VID:3344 PID:0001 VPC MongoosT-50CM3 (SN:N/A FW:VIRPIL Controls 20240101)"
    );

    // Test a device with empty firmware
    let device = VpcDevice {
        full_name: "3344:0001:123456".to_string(),
        name: Rc::new("VPC MongoosT-50CM3".to_string()),
        firmware: Rc::new("".to_string()),
        vendor_id: 0x3344,
        product_id: 0x0001,
        serial_number: "123456".to_string(),
        usage: 0,
        active: false,
    };

    assert_eq!(
        format!("{}", device),
        "VID:3344 PID:0001 VPC MongoosT-50CM3 (SN:123456 FW:N/A)"
    );
}
