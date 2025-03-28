use serde::{Deserialize, Serialize};
use std::ops::{Index, IndexMut};

// Configuration data saved to JSON
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigData {
    #[serde(default)] // Ensure field exists even if missing in JSON
    pub sources: Vec<crate::device::SavedDevice>,
    #[serde(default)]
    pub receivers: Vec<crate::device::SavedDevice>,
    #[serde(default)] // Use default if missing
    pub shift_modifiers: ModifiersArray,
}

// Default values for a new configuration
impl Default for ConfigData {
    fn default() -> Self {
        Self {
            sources: vec![], // Start with no sources configured
            receivers: vec![],
            shift_modifiers: ModifiersArray::default(), // Defaults to all OR
        }
    }
}

// Enum for shift modifier logic
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShiftModifiers {
    OR = 0,
    AND = 1,
    XOR = 2,
}

// How the modifier is displayed in the UI
impl std::fmt::Display for ShiftModifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ShiftModifiers::OR => write!(f, "OR"),
            ShiftModifiers::AND => write!(f, "AND"),
            ShiftModifiers::XOR => write!(f, "XOR"),
        }
    }
}

// Wrapper for the array of modifiers to implement Default and Indexing
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct ModifiersArray {
    data: [ShiftModifiers; 8],
}

impl Default for ModifiersArray {
    fn default() -> Self {
        Self {
            data: [ShiftModifiers::OR; 8], // Default to OR for all 8 bits
        }
    }
}

// Allow indexing like `modifiers_array[i]`
impl Index<usize> for ModifiersArray {
    type Output = ShiftModifiers;

    fn index(&self, index: usize) -> &ShiftModifiers {
        &self.data[index]
    }
}

// Allow mutable indexing like `modifiers_array[i] = ...`
impl IndexMut<usize> for ModifiersArray {
    fn index_mut(&mut self, index: usize) -> &mut ShiftModifiers {
        &mut self.data[index]
    }
}
