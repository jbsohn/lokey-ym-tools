pub mod delta;
pub mod player;
pub mod registers;
pub mod sequence;
pub mod timing;

pub use delta::DeltaCompiler;
pub use player::AudioPlayer;
pub use registers::YmRegisters;
pub use sequence::{SfxFrame, SfxSequence, YmChannel, YmFrame, YmSequence};
pub use timing::{SystemHz, TimingConfig};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_defaults() {
        let hz = SystemHz::Hz50;
        assert_eq!(hz.hz_value(), 50);
        assert_eq!(hz.frame_duration_ms(), 20.0);

        let hz60 = SystemHz::Hz60;
        assert_eq!(hz60.hz_value(), 60);
    }

    #[test]
    fn test_delta_compiler_basic() {
        let mut seq = SfxSequence {
            name: "test_sfx".to_string(),
            source_clock: 2_000_000,
            source_hz: 50,
            priority: 0,
            preferred_channels: None,
            loop_start: None,
            frames: Vec::new(),
        };
        seq.frames.push(SfxFrame {
            tone: Some(450),
            volume: Some(15),
            ..Default::default()
        });

        let compiler = DeltaCompiler::new();
        let payload = compiler.compile_sfx(&seq);
        assert_eq!(payload.len(), 5);
        assert_eq!(payload[0], 194); // 450 & 0xFF = 194
    }

    #[test]
    fn test_ayfx_csv_parsing() {
        let csv_data = "0,1,0x8a8,0x1f,0xf\n0,1,0x8a8,0x1c,0xe";
        let seq = SfxSequence::from_ayfx_csv("laser", csv_data).unwrap();

        assert_eq!(seq.name, "laser");
        assert_eq!(seq.source_clock, 2_000_000);
        assert_eq!(seq.source_hz, 50);
        assert_eq!(seq.frames.len(), 2);

        let frame = &seq.frames[0];
        assert_eq!(frame.tone_enable, Some(false));
        assert_eq!(frame.noise_enable, Some(true));
        assert_eq!(frame.tone, Some(2216)); // 0x8a8 = 2216
        assert_eq!(frame.noise, Some(31)); // 0x1f = 31
        assert_eq!(frame.volume, Some(15)); // 0xf = 15
    }

    #[test]
    fn test_ayfx_bank_parsing() {
        let bank_bytes = vec![
            1, 1, 0, 237, 31, 0, 0, 173, 37, 0, 172, 43, 0, 172, 49, 0, 172, 55, 0, 172, 61, 0,
            172, 67, 0, 172, 73, 0, 172, 79, 0, 172, 85, 0, 172, 91, 0, 172, 97, 0, 172, 103, 0,
            172, 109, 0, 172, 115, 0, 172, 121, 0, 172, 127, 0, 172, 133, 0, 172, 139, 0, 172, 145,
            0, 171, 151, 0, 170, 157, 0, 169, 163, 0, 168, 169, 0, 167, 175, 0, 166, 181, 0, 165,
            187, 0, 164, 193, 0, 163, 199, 0, 162, 205, 0, 161, 211, 0, 208, 32, 119, 105, 122, 98,
            97, 108, 108, 95, 49, 0,
        ];
        let bank = SfxSequence::from_ayfx_bank(&bank_bytes).unwrap();
        assert_eq!(bank.len(), 1);
        let seq = &bank[0];
        assert_eq!(seq.name, "wizball_1");
        assert_eq!(seq.frames.len(), 31);
        assert_eq!(seq.frames[0].volume, Some(13));
        assert_eq!(seq.frames[0].tone_enable, Some(true));
        assert_eq!(seq.frames[0].noise_enable, Some(false));
        assert_eq!(seq.frames[0].tone, Some(31));
        assert_eq!(seq.frames[0].noise, Some(0));
    }

    #[test]
    fn test_from_yfx() {
        let source_seq = SfxSequence {
            name: "test_sfx".to_string(),
            source_clock: 2_000_000,
            source_hz: 50,
            priority: 1,
            preferred_channels: None,
            loop_start: None,
            frames: vec![
                SfxFrame {
                    tone_enable: Some(true),
                    noise_enable: Some(false),
                    tone: Some(100),
                    noise: Some(0),
                    volume: Some(15),
                    duration: Some(1),
                },
                SfxFrame {
                    tone_enable: Some(true),
                    noise_enable: Some(false),
                    tone: Some(102),
                    noise: Some(0),
                    volume: Some(14),
                    duration: Some(1),
                },
            ],
        };

        let compiler = DeltaCompiler::new();
        let payload = compiler.compile_sfx(&source_seq);

        let decoded = SfxSequence::from_yfx("test_sfx", &payload).unwrap();
        assert_eq!(decoded.frames.len(), 2);
        assert_eq!(decoded.frames[0].tone, Some(100));
        assert_eq!(decoded.frames[0].volume, Some(15));
        assert_eq!(decoded.frames[1].tone, Some(102));
        assert_eq!(decoded.frames[1].volume, Some(14));
    }

    #[test]
    fn test_ayfx_effect_parsing() {
        // Just the effect data slice from pew.afb (after byte 3, length 106 minus name)
        let effect_bytes = vec![
            237, 31, 0, 0, 173, 37, 0, 172, 43, 0, 172, 49, 0, 172, 55, 0, 172, 61, 0, 172, 67, 0,
            172, 73, 0, 172, 79, 0, 172, 85, 0, 172, 91, 0, 172, 97, 0, 172, 103, 0, 172, 109, 0,
            172, 115, 0, 172, 121, 0, 172, 127, 0, 172, 133, 0, 172, 139, 0, 172, 145, 0, 171, 151,
            0, 170, 157, 0, 169, 163, 0, 168, 169, 0, 167, 175, 0, 166, 181, 0, 165, 187, 0, 164,
            193, 0, 163, 199, 0, 162, 205, 0, 161, 211, 0, 208, 32,
        ];
        let seq = SfxSequence::from_ayfx_effect("pew", &effect_bytes).unwrap();
        assert_eq!(seq.name, "pew");
        assert_eq!(seq.frames.len(), 31);
        assert_eq!(seq.frames[0].volume, Some(13));
        assert_eq!(seq.frames[0].tone_enable, Some(true));
        assert_eq!(seq.frames[0].noise_enable, Some(false));
        assert_eq!(seq.frames[0].tone, Some(31));
        assert_eq!(seq.frames[0].noise, Some(0));
    }

    #[test]
    fn test_song_compilation_and_parsing() {
        let mut frames = Vec::new();
        // Create 70 frames to span beyond a 64-frame pattern block
        for i in 0..70 {
            frames.push(YmFrame {
                tone_a: Some(200 + i as u16),
                volume_a: Some(15),
                tone_enable_a: Some(true),
                ..Default::default()
            });
        }
        let song = YmSequence {
            name: "test_song".to_string(),
            timing: TimingConfig {
                master_clock_hz: 2_000_000,
                frame_rate: SystemHz::Hz50,
            },
            priority: 0,
            loop_start: None,
            frames,
        };

        let compiler = DeltaCompiler::new();
        let ysg_bytes = compiler.compile_song(&song);

        let chosen_size = ysg_bytes[0] as usize;
        let seq_len = ysg_bytes[2] as usize;

        let decoded = YmSequence::from_ysg("test_song", &ysg_bytes).unwrap();
        // Should be padded to a multiple of the chosen pattern size
        assert_eq!(decoded.frames.len(), chosen_size * seq_len);
        assert_eq!(decoded.frames[0].tone_a, Some(200));
        assert_eq!(decoded.frames[0].volume_a, Some(15));
        assert_eq!(decoded.frames[69].tone_a, Some(269));
    }
}
