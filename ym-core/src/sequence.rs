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
    pub envelope_period: Option<u16>,
    pub envelope_shape: Option<u8>,
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

    pub fn from_ysg(name: &str, bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if bytes.len() < 12 {
            return Err("YSG file too small to contain header".into());
        }

        let pattern_size = bytes[0] as usize;
        let num_unique = bytes[1] as usize;
        let seq_len = bytes[2] as usize;
        let loop_pattern = bytes[3] as usize;
        let frame_rate_hz = u32::from_le_bytes(bytes[4..8].try_into()?);
        let master_clock_hz = u32::from_le_bytes(bytes[8..12].try_into()?);

        let seq_table_start = 12;
        let offset_table_start = seq_table_start + seq_len;
        let pattern_data_start = offset_table_start + num_unique * 4;

        if bytes.len() < pattern_data_start {
            return Err("YSG file truncated before pattern data".into());
        }

        // Read sequence table
        let mut sequence_table = Vec::with_capacity(seq_len);
        for i in 0..seq_len {
            sequence_table.push(bytes[seq_table_start + i] as usize);
        }

        // Read offset table
        let mut offsets = Vec::with_capacity(num_unique);
        for i in 0..num_unique {
            let ptr = offset_table_start + i * 4;
            let offset = bytes[ptr] as u32
                | ((bytes[ptr + 1] as u32) << 8)
                | ((bytes[ptr + 2] as u32) << 16)
                | ((bytes[ptr + 3] as u32) << 24);
            offsets.push(offset as usize);
        }

        let mut frames = Vec::new();

        // We reconstruct the linear frames
        for &pattern_idx in &sequence_table {
            if pattern_idx >= num_unique {
                return Err(format!(
                    "Sequence index {} out of range (max {})",
                    pattern_idx, num_unique
                )
                .into());
            }

            let start_ptr = pattern_data_start + offsets[pattern_idx];
            let mut pp = start_ptr;

            let mut registers = [0u8; 14];

            for _ in 0..pattern_size {
                if pp + 1 >= bytes.len() {
                    return Err("Unexpected EOF in YSG pattern data".into());
                }
                let mask = bytes[pp] as u16 | ((bytes[pp + 1] as u16) << 8);
                pp += 2;

                for (reg, slot) in registers.iter_mut().enumerate() {
                    if (mask & (1 << reg)) != 0 {
                        if pp >= bytes.len() {
                            return Err("Unexpected EOF in YSG pattern register payload".into());
                        }
                        *slot = bytes[pp];
                        pp += 1;
                    }
                }

                let tone_a = registers[0] as u16 | ((registers[1] as u16) << 8);
                let tone_b = registers[2] as u16 | ((registers[3] as u16) << 8);
                let tone_c = registers[4] as u16 | ((registers[5] as u16) << 8);
                let noise_period = registers[6];
                let mixer = registers[7];
                let volume_a = registers[8];
                let volume_b = registers[9];
                let volume_c = registers[10];
                let env_period = registers[11] as u16 | ((registers[12] as u16) << 8);
                let env_shape = registers[13];

                frames.push(YmFrame {
                    tone_a: Some(tone_a),
                    tone_b: Some(tone_b),
                    tone_c: Some(tone_c),
                    noise_period: Some(noise_period),
                    volume_a: Some(volume_a),
                    volume_b: Some(volume_b),
                    volume_c: Some(volume_c),
                    tone_enable_a: Some((mixer & 0x01) == 0),
                    tone_enable_b: Some((mixer & 0x02) == 0),
                    tone_enable_c: Some((mixer & 0x04) == 0),
                    noise_enable_a: Some((mixer & 0x08) == 0),
                    noise_enable_b: Some((mixer & 0x10) == 0),
                    noise_enable_c: Some((mixer & 0x20) == 0),
                    envelope_period: Some(env_period),
                    envelope_shape: Some(env_shape),
                    duration: Some(1),
                });
            }
        }

        let loop_start = if loop_pattern == 255 {
            None
        } else {
            Some(loop_pattern * pattern_size)
        };

        Ok(Self {
            name: name.to_string(),
            timing: TimingConfig {
                master_clock_hz,
                frame_rate: SystemHz::Custom(frame_rate_hz),
            },
            priority: 0,
            loop_start,
            frames,
        })
    }

    pub fn from_ym_data(name: &str, ym_data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        use ym2149_common::{ChiptunePlayer, MetadataFields};
        use ym2149_ym_replayer::decompress_if_needed;
        use ym2149_ym_replayer::load_song;
        use ym2149_ym_replayer::player::PlaybackController;

        let decompressed = decompress_if_needed(ym_data)?;

        let mut source_clock = 2_000_000u32;
        if decompressed.len() >= 26
            && (&decompressed[0..4] == b"YM5!" || &decompressed[0..4] == b"YM6!")
        {
            source_clock = u32::from_be_bytes([
                decompressed[22],
                decompressed[23],
                decompressed[24],
                decompressed[25],
            ]);
        }

        let target_clock = 1_789_773u32;
        let ratio = target_clock as f64 / source_clock as f64;
        let apply_scaling = (ratio - 1.0).abs() > 0.0001;

        let (mut player, summary) = load_song(ym_data)?;
        let total_frames = summary.frame_count;
        let samples_per_frame = summary.samples_per_frame as usize;

        // Loop point is static file metadata, safe to read before `play()` advances
        // playback state.
        let loop_start = player
            .metadata()
            .loop_frame()
            .filter(|&frame| frame < total_frames);
        PlaybackController::play(&mut player)?;

        let mut frames = Vec::with_capacity(total_frames);

        for _ in 0..total_frames {
            let regs = player.dump_registers();

            let tone_a = regs[0] as u16 | ((regs[1] as u16) << 8);
            let tone_b = regs[2] as u16 | ((regs[3] as u16) << 8);
            let tone_c = regs[4] as u16 | ((regs[5] as u16) << 8);
            let noise_period = regs[6];
            let mixer = regs[7];
            let volume_a = regs[8];
            let volume_b = regs[9];
            let volume_c = regs[10];
            let envelope_period = regs[11] as u16 | ((regs[12] as u16) << 8);
            let envelope_shape = regs[13];

            let mut frame = YmFrame {
                tone_a: Some(tone_a),
                tone_b: Some(tone_b),
                tone_c: Some(tone_c),
                noise_period: Some(noise_period),
                volume_a: Some(volume_a),
                volume_b: Some(volume_b),
                volume_c: Some(volume_c),
                tone_enable_a: Some((mixer & 0x01) == 0),
                tone_enable_b: Some((mixer & 0x02) == 0),
                tone_enable_c: Some((mixer & 0x04) == 0),
                noise_enable_a: Some((mixer & 0x08) == 0),
                noise_enable_b: Some((mixer & 0x10) == 0),
                noise_enable_c: Some((mixer & 0x20) == 0),
                envelope_period: Some(envelope_period),
                envelope_shape: Some(envelope_shape),
                duration: Some(1),
            };

            if apply_scaling {
                frame.scale_pitch(ratio);
            }

            frames.push(frame);

            // Advance player by one frame
            let mut dummy_buf = vec![0.0f32; samples_per_frame];
            player.generate_samples_into(&mut dummy_buf);
        }

        Ok(Self {
            name: name.to_string(),
            timing: TimingConfig {
                master_clock_hz: target_clock,
                frame_rate: SystemHz::Hz50,
            },
            priority: 0,
            loop_start,
            frames,
        })
    }

    /// Byte length of `ym_data` after decompression (e.g. from LHA-compressed
    /// `.ym` files), before any lokey-ym-tools recompilation. Useful for reporting
    /// how much smaller a compiled `.ysg` is than the source register stream.
    pub fn ym_decompressed_len(ym_data: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
        use ym2149_ym_replayer::decompress_if_needed;
        Ok(decompress_if_needed(ym_data)?.len())
    }
}

impl YmFrame {
    pub fn apply_to_chip(&self, chip: &mut impl ym2149::Ym2149Backend, mixer: &mut u8) {
        // Tone A (R0, R1)
        if let Some(tone) = self.tone_a {
            chip.write_register(0, (tone & 0xFF) as u8);
            chip.write_register(1, ((tone >> 8) & 0x0F) as u8);
        }
        // Tone B (R2, R3)
        if let Some(tone) = self.tone_b {
            chip.write_register(2, (tone & 0xFF) as u8);
            chip.write_register(3, ((tone >> 8) & 0x0F) as u8);
        }
        // Tone C (R4, R5)
        if let Some(tone) = self.tone_c {
            chip.write_register(4, (tone & 0xFF) as u8);
            chip.write_register(5, ((tone >> 8) & 0x0F) as u8);
        }
        // Noise Period (R6)
        if let Some(noise) = self.noise_period {
            chip.write_register(6, noise & 0x1F);
        }
        // Volume A, B, C (R8, R9, R10)
        if let Some(vol) = self.volume_a {
            chip.write_register(8, vol & 0x1F);
        }
        if let Some(vol) = self.volume_b {
            chip.write_register(9, vol & 0x1F);
        }
        if let Some(vol) = self.volume_c {
            chip.write_register(10, vol & 0x1F);
        }

        // Mixer Enable bits (R7) - 0 is ENABLED, 1 is DISABLED
        if let Some(en) = self.tone_enable_a {
            if en {
                *mixer &= !0x01;
            } else {
                *mixer |= 0x01;
            }
        }
        if let Some(en) = self.tone_enable_b {
            if en {
                *mixer &= !0x02;
            } else {
                *mixer |= 0x02;
            }
        }
        if let Some(en) = self.tone_enable_c {
            if en {
                *mixer &= !0x04;
            } else {
                *mixer |= 0x04;
            }
        }
        if let Some(en) = self.noise_enable_a {
            if en {
                *mixer &= !0x08;
            } else {
                *mixer |= 0x08;
            }
        }
        if let Some(en) = self.noise_enable_b {
            if en {
                *mixer &= !0x10;
            } else {
                *mixer |= 0x10;
            }
        }
        if let Some(en) = self.noise_enable_c {
            if en {
                *mixer &= !0x20;
            } else {
                *mixer |= 0x20;
            }
        }
        chip.write_register(7, *mixer);

        // Envelope Period (R11, R12)
        if let Some(period) = self.envelope_period {
            chip.write_register(11, (period & 0xFF) as u8);
            chip.write_register(12, ((period >> 8) & 0xFF) as u8);
        }

        // Envelope Shape (R13)
        if let Some(shape) = self.envelope_shape {
            let current_shape = chip.read_register(13);
            if current_shape != (shape & 0x0F) {
                chip.write_register(13, shape & 0x0F);
            }
        }
    }

    pub fn scale_pitch(&mut self, ratio: f64) {
        if let Some(t) = self.tone_a {
            self.tone_a = Some((t as f64 * ratio).round() as u16);
        }
        if let Some(t) = self.tone_b {
            self.tone_b = Some((t as f64 * ratio).round() as u16);
        }
        if let Some(t) = self.tone_c {
            self.tone_c = Some((t as f64 * ratio).round() as u16);
        }
        if let Some(n) = self.noise_period {
            self.noise_period = Some(((n & 0x1F) as f64 * ratio).round() as u8);
        }
        if let Some(e) = self.envelope_period {
            self.envelope_period = Some((e as f64 * ratio).round() as u16);
        }
    }
}

/// Sound channel selector for routing dynamic SFX.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum YmChannel {
    A,
    B,
    C,
}

/// Single-channel Sound Effect Frame, matching the validation schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SfxFrame {
    pub tone_enable: Option<bool>,
    pub noise_enable: Option<bool>,
    pub tone: Option<u16>,
    pub noise: Option<u8>,
    pub volume: Option<u8>,
    pub duration: Option<u8>,
}

/// Channel-agnostic Sound Effect manifest matching sfx-schema.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SfxSequence {
    pub name: String,
    pub source_clock: u32,
    pub source_hz: u32,
    pub priority: u8,
    pub preferred_channels: Option<Vec<YmChannel>>,
    pub loop_start: Option<usize>,
    pub frames: Vec<SfxFrame>,
}

impl SfxFrame {
    pub fn apply_to_chip(
        &self,
        chip: &mut impl ym2149::Ym2149Backend,
        mixer: &mut u8,
        channel: YmChannel,
    ) {
        let tone_reg_low = match channel {
            YmChannel::A => 0,
            YmChannel::B => 2,
            YmChannel::C => 4,
        };
        let tone_reg_high = tone_reg_low + 1;

        if let Some(t) = self.tone {
            chip.write_register(tone_reg_low, (t & 0xFF) as u8);
            chip.write_register(tone_reg_high, ((t >> 8) & 0x0F) as u8);
        }

        if let Some(n) = self.noise {
            chip.write_register(6, n & 0x1F);
        }

        let vol_reg = match channel {
            YmChannel::A => 8,
            YmChannel::B => 9,
            YmChannel::C => 10,
        };

        if let Some(v) = self.volume {
            chip.write_register(vol_reg, v & 0x1F);
        }

        let tone_bit = match channel {
            YmChannel::A => 0x01,
            YmChannel::B => 0x02,
            YmChannel::C => 0x04,
        };
        let noise_bit = match channel {
            YmChannel::A => 0x08,
            YmChannel::B => 0x10,
            YmChannel::C => 0x20,
        };

        if let Some(en) = self.tone_enable {
            if en {
                *mixer &= !tone_bit;
            } else {
                *mixer |= tone_bit;
            }
        }
        if let Some(en) = self.noise_enable {
            if en {
                *mixer &= !noise_bit;
            } else {
                *mixer |= noise_bit;
            }
        }
        chip.write_register(7, *mixer);
    }
}

impl SfxSequence {
    pub fn from_ayfx_csv(name: &str, content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut frames = Vec::new();
        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() < 5 {
                return Err(format!(
                    "Line {}: expected at least 5 columns, found {}",
                    line_num + 1,
                    parts.len()
                )
                .into());
            }

            let t = parts[0].parse::<i32>()? != 0;
            let n = parts[1].parse::<i32>()? != 0;

            let parse_val = |s: &str| -> Result<u16, Box<dyn std::error::Error>> {
                if let Some(hex) = s.strip_prefix("0x") {
                    Ok(u16::from_str_radix(hex, 16)?)
                } else if let Some(hex) = s.strip_prefix("0X") {
                    Ok(u16::from_str_radix(hex, 16)?)
                } else {
                    Ok(s.parse::<u16>()?)
                }
            };

            let tone = parse_val(parts[2])?;
            let noise = parse_val(parts[3])? as u8;
            let volume = parse_val(parts[4])? as u8;

            frames.push(SfxFrame {
                tone_enable: Some(t),
                noise_enable: Some(n),
                tone: Some(tone),
                noise: Some(noise),
                volume: Some(volume),
                duration: Some(1),
            });
        }

        Ok(Self {
            name: name.to_string(),
            source_clock: 2_000_000,
            source_hz: 50,
            priority: 1,
            preferred_channels: None,
            loop_start: None,
            frames,
        })
    }

    pub fn from_ayfx_bank(bank_data: &[u8]) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        if bank_data.is_empty() {
            return Err("Empty bank data".into());
        }

        let num_effects = bank_data[0] as usize;
        let mut sequences = Vec::new();

        for i in 0..num_effects {
            let offset_ptr = 1 + i * 2;
            if offset_ptr + 1 >= bank_data.len() {
                break;
            }
            let offset_val =
                (bank_data[offset_ptr] as u16 | ((bank_data[offset_ptr + 1] as u16) << 8)) as usize;
            let start_idx = 2 + i * 2 + offset_val;
            if start_idx >= bank_data.len() {
                continue;
            }

            // Determine max length for decode
            let max_len = if i < num_effects - 1 {
                let next_ptr = 3 + i * 2;
                if next_ptr + 1 < bank_data.len() {
                    let next_offset_val = (bank_data[next_ptr] as u16
                        | ((bank_data[next_ptr + 1] as u16) << 8))
                        as usize;
                    let next_start_idx = 4 + i * 2 + next_offset_val;
                    if next_start_idx > start_idx && next_start_idx <= bank_data.len() {
                        next_start_idx - start_idx
                    } else {
                        bank_data.len() - start_idx
                    }
                } else {
                    bank_data.len() - start_idx
                }
            } else {
                bank_data.len() - start_idx
            };

            let mut frames = Vec::new();
            let mut pp = start_idx;
            let end_limit = start_idx + max_len;

            let mut tone = 0u16;
            let mut noise = 0u8;

            while pp < end_limit {
                let it = bank_data[pp];
                pp += 1;

                if (it & (1 << 5)) != 0 {
                    if pp + 1 >= end_limit {
                        break;
                    }
                    tone = (bank_data[pp] as u16 | ((bank_data[pp + 1] as u16) << 8)) & 0xFFF;
                    pp += 2;
                }
                if (it & (1 << 6)) != 0 {
                    if pp >= end_limit {
                        break;
                    }
                    let n_val = bank_data[pp];
                    pp += 1;

                    if it == 0xD0 && n_val >= 0x20 {
                        break;
                    }
                    noise = n_val & 0x1F;
                }

                let vol = it & 0x0F;
                let t_enable = (it & (1 << 4)) == 0;
                let n_enable = (it & (1 << 7)) == 0;

                frames.push(SfxFrame {
                    tone_enable: Some(t_enable),
                    noise_enable: Some(n_enable),
                    tone: Some(tone),
                    noise: Some(noise),
                    volume: Some(vol),
                    duration: Some(1),
                });
            }

            // Parse optional null-terminated name if present in remaining space of effect block
            let mut name = format!("wizball_{}", i + 1);
            if pp < end_limit {
                let mut name_bytes = Vec::new();
                while pp < end_limit && bank_data[pp] != 0 {
                    name_bytes.push(bank_data[pp]);
                    pp += 1;
                }
                if !name_bytes.is_empty() {
                    if let Ok(decoded_name) = String::from_utf8(name_bytes) {
                        name = decoded_name;
                    }
                }
            }

            sequences.push(SfxSequence {
                name,
                source_clock: 2_000_000,
                source_hz: 50,
                priority: 1,
                preferred_channels: None,
                loop_start: None,
                frames,
            });
        }

        Ok(sequences)
    }

    pub fn from_ayfx_effect(name: &str, bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let mut frames = Vec::new();
        let mut pp = 0;
        let mut tone = 0u16;
        let mut noise = 0u8;
        let end_limit = bytes.len();

        while pp < end_limit {
            let it = bytes[pp];
            pp += 1;

            if (it & (1 << 5)) != 0 {
                if pp + 1 >= end_limit {
                    break;
                }
                tone = (bytes[pp] as u16 | ((bytes[pp + 1] as u16) << 8)) & 0xFFF;
                pp += 2;
            }
            if (it & (1 << 6)) != 0 {
                if pp >= end_limit {
                    break;
                }
                let n_val = bytes[pp];
                pp += 1;

                if it == 0xD0 && n_val >= 0x20 {
                    break;
                }
                noise = n_val & 0x1F;
            }

            let vol = it & 0x0F;
            let t_enable = (it & (1 << 4)) == 0;
            let n_enable = (it & (1 << 7)) == 0;

            frames.push(SfxFrame {
                tone_enable: Some(t_enable),
                noise_enable: Some(n_enable),
                tone: Some(tone),
                noise: Some(noise),
                volume: Some(vol),
                duration: Some(1),
            });
        }

        Ok(Self {
            name: name.to_string(),
            source_clock: 2_000_000,
            source_hz: 50,
            priority: 1,
            preferred_channels: None,
            loop_start: None,
            frames,
        })
    }

    pub fn from_yfx(name: &str, bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if !bytes.len().is_multiple_of(5) {
            return Err("YFX file size must be a multiple of 5".into());
        }

        let mut frames = Vec::new();
        let mut pp = 0;

        while pp < bytes.len() {
            let tone_low = bytes[pp];
            let tone_high = bytes[pp + 1];
            let volume = bytes[pp + 2];
            let control = bytes[pp + 3];
            let duration = bytes[pp + 4];
            pp += 5;

            let tone = tone_low as u16 | ((tone_high as u16) << 8);
            let tone_enable = (control & 0x01) != 0;
            let noise_enable = (control & 0x02) != 0;
            let noise = (control >> 3) & 0x1F;

            frames.push(SfxFrame {
                tone_enable: Some(tone_enable),
                noise_enable: Some(noise_enable),
                tone: Some(tone),
                noise: Some(noise),
                volume: Some(volume),
                duration: Some(duration),
            });
        }

        Ok(Self {
            name: name.to_string(),
            source_clock: 2_000_000,
            source_hz: 50,
            priority: 1,
            preferred_channels: None,
            loop_start: None,
            frames,
        })
    }
}
