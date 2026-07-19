use crate::sequence::YmSequence;

/// Platform-agnostic delta-mask compiler for YM-2149 register updates.
#[derive(Debug, Default)]
pub struct DeltaCompiler;

impl DeltaCompiler {
    pub fn new() -> Self {
        Self
    }

    /// Compiles a sequence into a platform-agnostic byte payload using delta masks.
    pub fn compile(&self, sequence: &YmSequence) -> Vec<u8> {
        let mut compiled_bytes = Vec::new();
        let mut active_tone_a: Option<u16> = None;
        let mut active_volume_a: Option<u8> = None;

        for frame in &sequence.frames {
            let mut mask: u8 = 0x00;
            let mut payload = Vec::new();

            // Tone Channel A changes
            if let Some(new_tone) = frame.tone_a {
                let current_low = (new_tone & 0xFF) as u8;
                let current_high = ((new_tone >> 8) & 0x0F) as u8;

                match active_tone_a {
                    Some(prev) => {
                        let prev_low = (prev & 0xFF) as u8;
                        let prev_high = ((prev >> 8) & 0x0F) as u8;

                        if current_low != prev_low {
                            mask |= 0x01;
                            payload.push(current_low);
                        }
                        if current_high != prev_high {
                            mask |= 0x02;
                            payload.push(current_high);
                        }
                    }
                    None => {
                        mask |= 0x01;
                        payload.push(current_low);
                        mask |= 0x02;
                        payload.push(current_high);
                    }
                }
                active_tone_a = Some(new_tone);
            }

            // Volume Channel A changes
            if let Some(new_vol) = frame.volume_a {
                let current_vol = new_vol & 0x0F;
                match active_volume_a {
                    Some(prev) => {
                        if current_vol != prev {
                            mask |= 0x04;
                            payload.push(current_vol);
                        }
                    }
                    None => {
                        mask |= 0x04;
                        payload.push(current_vol);
                    }
                }
                active_volume_a = Some(current_vol);
            }

            compiled_bytes.push(mask);
            compiled_bytes.extend(payload);
        }

        compiled_bytes
    }
}
