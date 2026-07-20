use crate::sequence::{SfxSequence, YmSequence};

/// Platform-agnostic delta-mask compiler for YM-2149 register updates.
#[derive(Debug, Default)]
pub struct DeltaCompiler;

impl DeltaCompiler {
    pub fn new() -> Self {
        Self
    }

    /// Compiles a sound effect sequence into a 5-byte fixed-width frame representation.
    pub fn compile_sfx(&self, sequence: &SfxSequence) -> Vec<u8> {
        let mut compiled_bytes = Vec::new();

        let mut active_tone = 0u16;
        let mut active_volume = 0u8;
        let mut active_tone_enable = true;
        let mut active_noise_enable = false;
        let mut active_noise_period = 0u8;

        for frame in &sequence.frames {
            if let Some(t) = frame.tone {
                active_tone = t;
            }
            if let Some(v) = frame.volume {
                active_volume = v;
            }
            if let Some(te) = frame.tone_enable {
                active_tone_enable = te;
            }
            if let Some(ne) = frame.noise_enable {
                active_noise_enable = ne;
            }
            if let Some(n) = frame.noise {
                active_noise_period = n;
            }

            let tone_low = (active_tone & 0xFF) as u8;
            let tone_high = ((active_tone >> 8) & 0x0F) as u8;

            let mut control = 0u8;
            if active_tone_enable {
                control |= 0x01;
            }
            if active_noise_enable {
                control |= 0x02;
            }
            control |= (active_noise_period & 0x1F) << 3;

            let duration = frame.duration.unwrap_or(1);

            compiled_bytes.push(tone_low);
            compiled_bytes.push(tone_high);
            compiled_bytes.push(active_volume & 0x0F);
            compiled_bytes.push(control);
            compiled_bytes.push(duration);
        }

        compiled_bytes
    }

    /// Compiles a music song sequence into a pattern-deduplicated YSG binary payload,
    /// automatically searching for the pattern size that yields the smallest binary.
    pub fn compile_song(&self, sequence: &YmSequence) -> Vec<u8> {
        let sizes = [16, 32, 48, 64, 80, 96, 128, 160, 192, 255];
        let mut best_data: Option<Vec<u8>> = None;
        let mut best_size = 64;

        for &size in &sizes {
            if let Some(data) = self.compile_song_with_size(sequence, size) {
                if best_data.is_none() || data.len() < best_data.as_ref().unwrap().len() {
                    best_data = Some(data);
                    best_size = size;
                }
            }
        }

        let final_data = best_data.unwrap_or_else(|| {
            // fallback if all trials failed (e.g. song too long for small sizes)
            self.compile_song_with_size(sequence, 64)
                .unwrap_or_default()
        });

        println!(
            "Pattern size optimization: chose best size {} (final size: {} bytes)",
            best_size,
            final_data.len()
        );
        final_data
    }

    pub fn compile_song_with_size(
        &self,
        sequence: &YmSequence,
        pattern_size: usize,
    ) -> Option<Vec<u8>> {
        let total_frames = sequence.frames.len();
        if total_frames == 0 {
            return Some(Vec::new());
        }

        // Chunk frames into blocks of pattern_size, padding the last one
        let mut raw_blocks = Vec::new();
        let mut idx = 0;
        while idx < total_frames {
            let end = (idx + pattern_size).min(total_frames);
            let mut block_frames = sequence.frames[idx..end].to_vec();
            while block_frames.len() < pattern_size {
                block_frames.push(crate::sequence::YmFrame::default());
            }
            raw_blocks.push(block_frames);
            idx += pattern_size;
        }

        if raw_blocks.len() > 255 {
            // Sequence table cannot fit in 1 byte length or index range
            return None;
        }

        // Serialize each pattern block to binary bytes
        let mut serialized_blocks = Vec::new();
        for block in &raw_blocks {
            serialized_blocks.push(Self::serialize_ym_block(block));
        }

        // Deduplicate serialized pattern blocks
        let mut unique_patterns: Vec<Vec<u8>> = Vec::new();
        let mut sequence_table: Vec<u8> = Vec::new();

        for block in serialized_blocks {
            let position = unique_patterns.iter().position(|x| x == &block);
            match position {
                Some(p_idx) => {
                    sequence_table.push(p_idx as u8);
                }
                None => {
                    let new_idx = unique_patterns.len();
                    if new_idx >= 256 {
                        return None;
                    }
                    unique_patterns.push(block);
                    sequence_table.push(new_idx as u8);
                }
            }
        }

        // Build output YSG payload
        let num_unique = unique_patterns.len();
        let seq_len = sequence_table.len();

        let loop_pattern = match sequence.loop_start {
            Some(frame) => {
                let pat_idx = frame / pattern_size;
                if pat_idx < seq_len {
                    pat_idx as u8
                } else {
                    0
                }
            }
            None => 255, // 255 means no loop
        };

        let mut output = Vec::new();
        output.push(pattern_size as u8);
        output.push(num_unique as u8);
        output.push(seq_len as u8);
        output.push(loop_pattern);

        // Sequence Table
        output.extend(&sequence_table);

        // Offset Table
        let mut current_offset = 0usize;
        let mut offset_table = Vec::new();
        for pat in &unique_patterns {
            offset_table.push((current_offset & 0xFF) as u8);
            offset_table.push(((current_offset >> 8) & 0xFF) as u8);
            offset_table.push(((current_offset >> 16) & 0xFF) as u8);
            offset_table.push(((current_offset >> 24) & 0xFF) as u8);
            current_offset += pat.len();
        }
        output.extend(offset_table);

        // Pattern Data
        for pat in unique_patterns {
            output.extend(pat);
        }

        Some(output)
    }

    fn serialize_ym_block(frames: &[crate::sequence::YmFrame]) -> Vec<u8> {
        let mut data = Vec::new();
        let mut registers = [0u8; 14];

        for (idx, frame) in frames.iter().enumerate() {
            let mut new_registers = [0u8; 14];

            // Tone A
            let tone_a = frame.tone_a.unwrap_or(0);
            new_registers[0] = (tone_a & 0xFF) as u8;
            new_registers[1] = ((tone_a >> 8) & 0x0F) as u8;

            // Tone B
            let tone_b = frame.tone_b.unwrap_or(0);
            new_registers[2] = (tone_b & 0xFF) as u8;
            new_registers[3] = ((tone_b >> 8) & 0x0F) as u8;

            // Tone C
            let tone_c = frame.tone_c.unwrap_or(0);
            new_registers[4] = (tone_c & 0xFF) as u8;
            new_registers[5] = ((tone_c >> 8) & 0x0F) as u8;

            // Noise Period
            new_registers[6] = frame.noise_period.unwrap_or(0) & 0x1F;

            // Mixer R7
            let mut mixer = 0x3F;
            if frame.tone_enable_a.unwrap_or(true) {
                mixer &= !0x01;
            }
            if frame.tone_enable_b.unwrap_or(true) {
                mixer &= !0x02;
            }
            if frame.tone_enable_c.unwrap_or(true) {
                mixer &= !0x04;
            }
            if frame.noise_enable_a.unwrap_or(false) {
                mixer &= !0x08;
            }
            if frame.noise_enable_b.unwrap_or(false) {
                mixer &= !0x10;
            }
            if frame.noise_enable_c.unwrap_or(false) {
                mixer &= !0x20;
            }
            new_registers[7] = mixer;

            // Volumes
            new_registers[8] = frame.volume_a.unwrap_or(0) & 0x1F;
            new_registers[9] = frame.volume_b.unwrap_or(0) & 0x1F;
            new_registers[10] = frame.volume_c.unwrap_or(0) & 0x1F;

            // Envelopes
            let env_period = frame.envelope_period.unwrap_or(0);
            new_registers[11] = (env_period & 0xFF) as u8;
            new_registers[12] = ((env_period >> 8) & 0xFF) as u8;
            new_registers[13] = frame.envelope_shape.unwrap_or(0) & 0x0F;

            let mut mask = 0u16;
            let mut payload = Vec::new();

            if idx == 0 {
                // First frame writes ALL registers
                mask = 0x3FFF;
                for r in 0..14 {
                    payload.push(new_registers[r]);
                }
            } else {
                // Delta from previous frame
                for r in 0..14 {
                    if new_registers[r] != registers[r] {
                        mask |= 1 << r;
                        payload.push(new_registers[r]);
                    }
                }
            }

            registers = new_registers;

            data.push((mask & 0xFF) as u8);
            data.push(((mask >> 8) & 0xFF) as u8);
            data.extend(payload);
        }

        data
    }
}
