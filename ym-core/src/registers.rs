use serde::{Deserialize, Serialize};

/// Represents the 14 programmable registers of the YM-2149 Sound Generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct YmRegisters {
    pub fine_tone_a: u8,       // R0
    pub coarse_tone_a: u8,     // R1 (4 bits)
    pub fine_tone_b: u8,       // R2
    pub coarse_tone_b: u8,     // R3 (4 bits)
    pub fine_tone_c: u8,       // R4
    pub coarse_tone_c: u8,     // R5 (4 bits)
    pub noise_period: u8,      // R6 (5 bits)
    pub mixer_control: u8,     // R7 (Mixer: Tone/Noise enable bits)
    pub volume_a: u8,          // R8 (4 bits volume + envelope mode bit)
    pub volume_b: u8,          // R9 (4 bits volume + envelope mode bit)
    pub volume_c: u8,          // R10 (4 bits volume + envelope mode bit)
    pub env_period_fine: u8,   // R11
    pub env_period_coarse: u8, // R12
    pub env_shape_cycle: u8,   // R13 (4 bits)
}

impl YmRegisters {
    pub fn new() -> Self {
        Self::default()
    }
}
