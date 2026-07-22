use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Runs `f` behind an animated spinner labeled `message`, for CLI feedback around a
/// blocking call (e.g. chip-emulated decoding, pattern-size search) that has no
/// incremental progress to report.
pub fn with_spinner<T>(message: &str, f: impl FnOnce() -> T) -> T {
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template("{spinner:.green} {msg}").unwrap());
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));

    let result = f();

    pb.finish_and_clear();
    result
}
