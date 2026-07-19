pub mod delta;
pub mod registers;
pub mod sequence;
pub mod timing;

pub use delta::DeltaCompiler;
pub use registers::YmRegisters;
pub use sequence::{YmFrame, YmSequence};
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
        let mut seq = YmSequence::new("test_sfx", SystemHz::Hz50);
        seq.frames.push(YmFrame {
            tone_a: Some(450),
            volume_a: Some(15),
            ..Default::default()
        });

        let compiler = DeltaCompiler::new();
        let payload = compiler.compile(&seq);
        assert_eq!(payload.len(), 4);
        assert_eq!(payload[0], 0x07);
    }
}
