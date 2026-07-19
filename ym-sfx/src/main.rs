use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;
use ym_core::{DeltaCompiler, SystemHz, YmSequence};

#[derive(Parser, Debug)]
#[command(
    name = "ym-sfx",
    version,
    about = "YM-2149 Sound Effect Toolchain",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum HzOption {
    #[value(name = "50")]
    Hz50,
    #[value(name = "60")]
    Hz60,
}

impl From<HzOption> for SystemHz {
    fn from(opt: HzOption) -> Self {
        match opt {
            HzOption::Hz50 => SystemHz::Hz50,
            HzOption::Hz60 => SystemHz::Hz60,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Render a sound effect source file into compiled YM-2149 binary payload
    Render {
        /// Input source file path (.json, etc.)
        #[arg(short, long)]
        input: PathBuf,

        /// Output compiled binary file path (.ysb)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Timing refresh rate override (50 or 60 Hz)
        #[arg(long, value_enum)]
        hz: Option<HzOption>,
    },
    /// Audition and play a sound effect sequence
    Play {
        /// Input source file path or compiled binary path
        #[arg(short, long)]
        input: PathBuf,

        /// Timing refresh rate override (50 or 60 Hz)
        #[arg(long, value_enum)]
        hz: Option<HzOption>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Render { input, output, hz } => {
            let output_path = output.unwrap_or_else(|| {
                let mut path = input.clone();
                path.set_extension("ysb");
                path
            });

            let content = fs::read_to_string(&input)?;
            let mut sequence: YmSequence = serde_json::from_str(&content)?;

            if let Some(hz_override) = hz {
                sequence.timing.frame_rate = hz_override.into();
            }

            let compiler = DeltaCompiler::new();
            let binary = compiler.compile(&sequence);

            fs::write(&output_path, &binary)?;
            println!(
                "RENDER SUCCESS: Compiled {} frames -> {} ({} bytes, Rate: {} Hz)",
                sequence.frames.len(),
                output_path.display(),
                binary.len(),
                sequence.timing.frame_rate.hz_value()
            );
        }
        Commands::Play { input, hz } => {
            let hz_str = hz
                .map(|h| format!("{} Hz", SystemHz::from(h).hz_value()))
                .unwrap_or_else(|| "Default Hz".to_string());
            println!(
                "PLAY AUDITION: Loading {} for playback (Rate: {})...",
                input.display(),
                hz_str
            );
            // Engine playback integration point
        }
    }

    Ok(())
}
