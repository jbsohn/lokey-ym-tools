use serde::{Deserialize, Serialize};

/// Supported playback refresh rates for YM-2149 sound sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SystemHz {
    #[default]
    Hz50,
    Hz60,
    Custom(u32),
}

impl SystemHz {
    pub fn hz_value(&self) -> u32 {
        match self {
            SystemHz::Hz50 => 50,
            SystemHz::Hz60 => 60,
            SystemHz::Custom(hz) => *hz,
        }
    }

    pub fn frame_duration_ms(&self) -> f64 {
        1000.0 / (self.hz_value() as f64)
    }
}

/// Timing configuration for YM-2149 sound generation and playback.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimingConfig {
    pub master_clock_hz: u32,
    pub frame_rate: SystemHz,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            master_clock_hz: 2_000_000, // Standard 2.0 MHz YM-2149 clock
            frame_rate: SystemHz::Hz50,
        }
    }
}
