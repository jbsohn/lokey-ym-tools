use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;
use ym_core::{AudioPlayer, DeltaCompiler, SfxSequence, SystemHz};

#[derive(Parser, Debug)]
#[command(
    name = "ym-sfx",
    version,
    about = "YM-2149 Sound Effect Toolchain",
    long_about = None
)]
struct SfxCli {
    #[command(subcommand)]
    command: SfxCommands,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum SfxHzOption {
    #[value(name = "50")]
    Hz50,
    #[value(name = "60")]
    Hz60,
}

impl From<SfxHzOption> for SystemHz {
    fn from(opt: SfxHzOption) -> Self {
        match opt {
            SfxHzOption::Hz50 => SystemHz::Hz50,
            SfxHzOption::Hz60 => SystemHz::Hz60,
        }
    }
}

#[derive(Subcommand, Debug)]
enum SfxCommands {
    /// Render a sound effect source file into compiled YM-2149 binary payload
    Render {
        /// Input source file path (.json, etc.)
        #[arg(short, long)]
        input: PathBuf,

        /// Output compiled binary file path (.yfx)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Timing refresh rate override (50 or 60 Hz)
        #[arg(long, value_enum)]
        hz: Option<SfxHzOption>,
    },
    /// Audition and play a sound effect sequence
    Play {
        /// Input source file path or compiled binary path
        #[arg(short, long)]
        input: PathBuf,

        /// Timing refresh rate override (50 or 60 Hz)
        #[arg(long, value_enum)]
        hz: Option<SfxHzOption>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = SfxCli::parse();

    match cli.command {
        SfxCommands::Render { input, output, hz } => {
            let output_path = output.unwrap_or_else(|| {
                let mut path = input.clone();
                path.set_extension("yfx");
                path
            });

            let extension = input.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            let mut sequence = if extension == "csv" {
                let name = input.file_stem().and_then(|s| s.to_str()).unwrap_or("sfx");
                let content = fs::read_to_string(&input)?;
                SfxSequence::from_ayfx_csv(name, &content)?
            } else if extension == "afb" {
                let bytes = fs::read(&input)?;
                let bank = SfxSequence::from_ayfx_bank(&bytes)?;
                if bank.is_empty() {
                    return Err("AYFX bank contains no sound effects".into());
                }
                println!("Loaded AYFX bank with {} sound effects.", bank.len());
                bank[0].clone() // default to first effect
            } else if extension == "afx" {
                let name = input.file_stem().and_then(|s| s.to_str()).unwrap_or("sfx");
                let bytes = fs::read(&input)?;
                SfxSequence::from_ayfx_effect(name, &bytes)?
            } else {
                let content = fs::read_to_string(&input)?;
                serde_json::from_str(&content)?
            };

            if let Some(hz_override) = hz {
                sequence.source_hz = SystemHz::from(hz_override).hz_value();
            }

            let compiler = DeltaCompiler::new();
            let binary = compiler.compile_sfx(&sequence);

            fs::write(&output_path, &binary)?;
            println!(
                "RENDER SUCCESS: Compiled {} frames -> {} ({} bytes, Rate: {} Hz)",
                sequence.frames.len(),
                output_path.display(),
                binary.len(),
                sequence.source_hz
            );
        }
        SfxCommands::Play { input, hz } => {
            let extension = input.extension().and_then(|ext| ext.to_str()).unwrap_or("");
            let mut sequence = if extension == "csv" {
                let name = input.file_stem().and_then(|s| s.to_str()).unwrap_or("sfx");
                let content = fs::read_to_string(&input)?;
                SfxSequence::from_ayfx_csv(name, &content)?
            } else if extension == "afb" {
                let bytes = fs::read(&input)?;
                let bank = SfxSequence::from_ayfx_bank(&bytes)?;
                if bank.is_empty() {
                    return Err("AYFX bank contains no sound effects".into());
                }
                println!("Loaded AYFX bank with {} sound effects.", bank.len());
                bank[0].clone() // default to first effect
            } else if extension == "afx" {
                let name = input.file_stem().and_then(|s| s.to_str()).unwrap_or("sfx");
                let bytes = fs::read(&input)?;
                SfxSequence::from_ayfx_effect(name, &bytes)?
            } else if extension == "yfx" {
                let name = input.file_stem().and_then(|s| s.to_str()).unwrap_or("sfx");
                let bytes = fs::read(&input)?;
                SfxSequence::from_yfx(name, &bytes)?
            } else {
                let content = fs::read_to_string(&input)?;
                serde_json::from_str(&content)?
            };

            if let Some(hz_override) = hz {
                sequence.source_hz = SystemHz::from(hz_override).hz_value();
            }

            println!(
                "PLAY AUDITION: Loading {} for playback (Rate: {} Hz)...",
                input.display(),
                sequence.source_hz
            );

            AudioPlayer::play_sfx(&sequence)?;
        }
    }

    Ok(())
}
