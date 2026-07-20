use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;
use ym_core::{AudioPlayer, DeltaCompiler, SystemHz, YmSequence};

#[derive(Parser, Debug)]
#[command(
    name = "ym-song",
    version,
    about = "YM-2149 Music Compilation & Auditioning Toolchain",
    long_about = None
)]
struct SongCli {
    #[command(subcommand)]
    command: SongCommands,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum SongHzOption {
    #[value(name = "50")]
    Hz50,
    #[value(name = "60")]
    Hz60,
}

impl From<SongHzOption> for SystemHz {
    fn from(opt: SongHzOption) -> Self {
        match opt {
            SongHzOption::Hz50 => SystemHz::Hz50,
            SongHzOption::Hz60 => SystemHz::Hz60,
        }
    }
}

#[derive(Subcommand, Debug)]
enum SongCommands {
    /// Render a music song file into compiled YM-2149 binary stream
    Render {
        /// Input music source file path (.json, etc.)
        #[arg(short, long)]
        input: PathBuf,

        /// Output compiled binary file path (.ym)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Timing refresh rate override (50 or 60 Hz)
        #[arg(long, value_enum)]
        hz: Option<SongHzOption>,

        /// Frame step (downsample rate: e.g. 2 to skip every other frame)
        #[arg(short, long, default_value_t = 1)]
        step: usize,

        /// Maximum frames to process (cuts off song after this limit)
        #[arg(short, long)]
        max_frames: Option<usize>,
    },
    /// Audition and play a music song file or stream
    Play {
        /// Input music source file path or compiled binary path
        #[arg(short, long)]
        input: PathBuf,

        /// Timing refresh rate override (50 or 60 Hz)
        #[arg(long, value_enum)]
        hz: Option<SongHzOption>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = SongCli::parse();

    match cli.command {
        SongCommands::Render {
            input,
            output,
            hz,
            step,
            max_frames,
        } => {
            let output_path = output.unwrap_or_else(|| {
                let mut path = input.clone();
                path.set_extension("ysg");
                path
            });

            let extension = input.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            let name = input.file_stem().and_then(|s| s.to_str()).unwrap_or("song");

            let mut sequence = if extension.eq_ignore_ascii_case("ym") {
                let bytes = fs::read(&input)?;
                YmSequence::from_ym_data(name, &bytes)?
            } else {
                let content = fs::read_to_string(&input)?;
                serde_json::from_str(&content)?
            };

            // Apply frame limit and step decimation
            let limit = max_frames
                .unwrap_or(sequence.frames.len())
                .min(sequence.frames.len());
            let mut decimated_frames = Vec::new();
            let mut i = 0;
            while i < limit {
                let window_end = (i + step).min(limit);
                let mut final_frame = sequence.frames[i].clone();

                // Peak volume detector over the step window
                let mut max_vol_a = 0u8;
                let mut max_vol_b = 0u8;
                let mut max_vol_c = 0u8;

                for idx in i..window_end {
                    let f = &sequence.frames[idx];
                    max_vol_a = max_vol_a.max(f.volume_a.unwrap_or(0));
                    max_vol_b = max_vol_b.max(f.volume_b.unwrap_or(0));
                    max_vol_c = max_vol_c.max(f.volume_c.unwrap_or(0));
                }

                final_frame.volume_a = Some(max_vol_a);
                final_frame.volume_b = Some(max_vol_b);
                final_frame.volume_c = Some(max_vol_c);

                decimated_frames.push(final_frame);
                i += step;
            }
            sequence.frames = decimated_frames;

            if let Some(hz_override) = hz {
                sequence.timing.frame_rate = hz_override.into();
            } else if step > 1 {
                let current_hz = sequence.timing.frame_rate.hz_value();
                sequence.timing.frame_rate =
                    ym_core::timing::SystemHz::Custom(current_hz / step as u32);
            }

            let compiler = DeltaCompiler::new();
            let binary = compiler.compile_song(&sequence);

            fs::write(&output_path, &binary)?;
            println!(
                "SONG RENDER SUCCESS: Compiled {} frames -> {} ({} bytes, Rate: {} Hz)",
                sequence.frames.len(),
                output_path.display(),
                binary.len(),
                sequence.timing.frame_rate.hz_value()
            );
        }
        SongCommands::Play { input, hz } => {
            let extension = input.extension().and_then(|ext| ext.to_str()).unwrap_or("");

            if extension == "json" {
                let content = fs::read_to_string(&input)?;
                let mut sequence: YmSequence = serde_json::from_str(&content)?;

                if let Some(hz_override) = hz {
                    sequence.timing.frame_rate = hz_override.into();
                }

                println!(
                    "SONG PLAY AUDITION: Playing {} (Rate: {} Hz)...",
                    input.display(),
                    sequence.timing.frame_rate.hz_value()
                );

                AudioPlayer::play(&sequence)?;
            } else if extension == "ysg" {
                let name = input.file_stem().and_then(|s| s.to_str()).unwrap_or("song");
                let bytes = fs::read(&input)?;
                let mut sequence = YmSequence::from_ysg(name, &bytes)?;

                if let Some(hz_override) = hz {
                    sequence.timing.frame_rate = hz_override.into();
                }

                println!(
                    "SONG PLAY AUDITION: Playing compiled song {} (Rate: {} Hz)...",
                    input.display(),
                    sequence.timing.frame_rate.hz_value()
                );

                AudioPlayer::play(&sequence)?;
            } else {
                println!(
                    "SONG PLAY AUDITION: Playing chiptune song {}...",
                    input.display()
                );
                let ym_data = fs::read(&input)?;
                AudioPlayer::play_ym_data(&ym_data)?;
            }
        }
    }

    Ok(())
}
