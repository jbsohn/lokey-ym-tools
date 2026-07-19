use crate::timing::{SystemHz, TimingConfig};
use serde::{Deserialize, Serialize};

/// High-level frame representation for YM-2149 sound sequence authoring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct YmFrame {
    pub tone_a: Option<u16>,
    pub tone_b: Option<u16>,
    pub tone_c: Option<u16>,
    pub noise_period: Option<u8>,
    pub volume_a: Option<u8>,
    pub volume_b: Option<u8>,
    pub volume_c: Option<u8>,
    pub tone_enable_a: Option<bool>,
    pub tone_enable_b: Option<bool>,
    pub tone_enable_c: Option<bool>,
    pub noise_enable_a: Option<bool>,
    pub noise_enable_b: Option<bool>,
    pub noise_enable_c: Option<bool>,
    pub duration: Option<u8>,
}

/// Sound sequence manifest container for YM-2149 assets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YmSequence {
    pub name: String,
    pub timing: TimingConfig,
    pub priority: u8,
    pub loop_start: Option<usize>,
    pub frames: Vec<YmFrame>,
}

impl YmSequence {
    pub fn new(name: impl Into<String>, hz: SystemHz) -> Self {
        Self {
            name: name.into(),
            timing: TimingConfig {
                master_clock_hz: 2_000_000,
                frame_rate: hz,
            },
            priority: 0,
            loop_start: None,
            frames: Vec::new(),
        }
    }
}
