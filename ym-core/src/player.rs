use crate::sequence::{SfxFrame, SfxSequence, YmChannel, YmFrame, YmSequence};
use console::style;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ym2149::{Ym2149, Ym2149Backend};

pub struct AudioPlayer;

/// Progress bar styled for a known frame count (sfx/song playback from `.yfx`/`.ysg`).
fn frame_progress_bar(total_frames: u64) -> ProgressBar {
    let pb = ProgressBar::new(total_frames);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] frame {pos}/{len}{msg}",
        )
        .unwrap()
        .progress_chars("=>-"),
    );
    pb
}

/// Progress bar styled for elapsed-time playback (raw `.ym` chiptune data, where no
/// frame count is exposed by the replayer crate).
fn time_progress_bar(total_deciseconds: u64) -> ProgressBar {
    let pb = ProgressBar::new(total_deciseconds);
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} [{bar:40.cyan/blue}] {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb
}

/// All state touched by the audio callback, behind a single lock so each
/// callback invocation takes one mutex instead of one per field.
struct PlaybackState {
    chip: Ym2149,
    frame_idx: usize,
    sample_in_frame: usize,
    mixer: u8,
    finished: bool,
}

/// cpal output device/stream parameters, bundled so `build_stream` doesn't need
/// a separate argument for each one.
struct AudioSink<'a> {
    device: &'a cpal::Device,
    stream_config: cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    channels: usize,
}

type StreamResult = Result<(cpal::Stream, Arc<Mutex<PlaybackState>>), Box<dyn std::error::Error>>;

impl AudioPlayer {
    /// Builds and starts a cpal output stream that clocks `chip` and feeds it one
    /// frame of `F` at a time via `apply`, advancing every `samples_per_frame`
    /// samples and looping back to `loop_start` (or finishing) at the end.
    ///
    /// Shared by `play_sfx` and `play`, which only differ in how a frame gets
    /// applied to the chip and in what happens after the stream starts.
    fn build_stream<F, Apply>(
        sink: AudioSink,
        samples_per_frame: usize,
        mut chip: Ym2149,
        frames: Vec<F>,
        loop_start: Option<usize>,
        mut apply: Apply,
    ) -> StreamResult
    where
        F: Send + 'static,
        Apply: FnMut(&F, &mut Ym2149, &mut u8) + Send + 'static,
    {
        // Apply initial frame registers before the stream (and its callback) exist.
        let mut mixer = 0x3F; // Default: all tones & noise muted
        apply(&frames[0], &mut chip, &mut mixer);

        let total_frames = frames.len();
        let state = Arc::new(Mutex::new(PlaybackState {
            chip,
            frame_idx: 0,
            sample_in_frame: 0,
            mixer,
            finished: false,
        }));

        let state_cb = Arc::clone(&state);
        let frames_cb = frames;
        let channels = sink.channels;
        let err_fn = |err| eprintln!("{} {}", style("Audio stream error:").red().bold(), err);

        let stream = match sink.sample_format {
            cpal::SampleFormat::F32 => sink.device.build_output_stream(
                sink.stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut s = state_cb.lock().unwrap();

                    if s.finished {
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                        return;
                    }

                    let mut i = 0;
                    while i < data.len() {
                        let sample_val = s.chip.get_sample();
                        s.chip.clock();

                        for c in 0..channels {
                            if i + c < data.len() {
                                data[i + c] = sample_val;
                            }
                        }
                        i += channels;

                        s.sample_in_frame += 1;
                        if s.sample_in_frame >= samples_per_frame {
                            s.sample_in_frame = 0;
                            s.frame_idx += 1;

                            if s.frame_idx >= total_frames {
                                if let Some(l_start) = loop_start {
                                    s.frame_idx = l_start;
                                    let idx = s.frame_idx;
                                    let s = &mut *s;
                                    apply(&frames_cb[idx], &mut s.chip, &mut s.mixer);
                                } else {
                                    s.finished = true;
                                }
                            } else {
                                let idx = s.frame_idx;
                                let s = &mut *s;
                                apply(&frames_cb[idx], &mut s.chip, &mut s.mixer);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )?,
            _ => return Err("Unsupported audio sample format".into()),
        };

        Ok((stream, state))
    }

    pub fn play_sfx(sequence: &SfxSequence) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No default output audio device found")?;
        let config = device.default_output_config()?;

        let sample_rate = config.sample_rate();
        let channels = config.channels() as usize;
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        let chip = Ym2149::with_clocks(sequence.source_clock, sample_rate);

        let hz = sequence.source_hz;
        let samples_per_frame = (sample_rate as f64 / hz as f64).round() as usize;

        let frames = sequence.frames.clone();
        if frames.is_empty() {
            println!("{}", style("Sequence contains no frames to play.").yellow());
            return Ok(());
        }
        let total_frames = frames.len();

        let channel = sequence
            .preferred_channels
            .as_ref()
            .and_then(|c| c.first().copied())
            .unwrap_or(YmChannel::A);

        let sink = AudioSink {
            device: &device,
            stream_config,
            sample_format,
            channels,
        };
        let (stream, state) = Self::build_stream(
            sink,
            samples_per_frame,
            chip,
            frames,
            None,
            move |frame: &SfxFrame, chip, mixer| frame.apply_to_chip(chip, mixer, channel),
        )?;

        stream.play()?;

        println!(
            "{} '{}' ({} frames @ {} Hz on channel {:?})",
            style("PLAYING SOUND EFFECT:").bold().green(),
            sequence.name,
            total_frames,
            hz,
            channel
        );

        let pb = frame_progress_bar(total_frames as u64);
        loop {
            let (current_frame, is_done) = {
                let s = state.lock().map_err(|_| "playback state mutex poisoned")?;
                (s.frame_idx, s.finished)
            };
            pb.set_position(current_frame.min(total_frames) as u64);
            if is_done {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        pb.finish_and_clear();

        // Let the audio buffer finish draining what's already queued.
        std::thread::sleep(Duration::from_millis(100));

        Ok(())
    }

    pub fn play(sequence: &YmSequence) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No default output audio device found")?;
        let config = device.default_output_config()?;

        let sample_rate = config.sample_rate();
        let channels = config.channels() as usize;
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        let chip = Ym2149::with_clocks(sequence.timing.master_clock_hz, sample_rate);

        let hz = sequence.timing.frame_rate.hz_value();
        let samples_per_frame = (sample_rate as f64 / hz as f64).round() as usize;

        let frames = sequence.frames.clone();
        if frames.is_empty() {
            println!("{}", style("Sequence contains no frames to play.").yellow());
            return Ok(());
        }
        let total_frames = frames.len();
        let loop_start_val = sequence.loop_start;

        let sink = AudioSink {
            device: &device,
            stream_config,
            sample_format,
            channels,
        };
        let (stream, state) = Self::build_stream(
            sink,
            samples_per_frame,
            chip,
            frames,
            loop_start_val,
            |frame: &YmFrame, chip, mixer| frame.apply_to_chip(chip, mixer),
        )?;

        stream.play()?;

        println!(
            "{} '{}' ({} frames @ {} Hz)",
            style("PLAYING SONG:").bold().green(),
            sequence.name,
            total_frames,
            hz
        );

        let is_looping = loop_start_val.is_some();
        let pb = frame_progress_bar(total_frames as u64);
        if is_looping {
            pb.set_message(format!(" {}", style("(looping, Ctrl+C to stop)").yellow()));
        }

        // If looping, this runs until interrupted, matching the intent of an
        // audition tool: a looping song has no natural end.
        loop {
            let (current_frame, is_done) = {
                let s = state.lock().map_err(|_| "playback state mutex poisoned")?;
                (s.frame_idx, s.finished)
            };
            pb.set_position(current_frame.min(total_frames) as u64);
            if is_done {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        pb.finish_and_clear();

        Ok(())
    }

    pub fn play_ym_data(ym_data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        use ym2149_common::ChiptunePlayerBase;
        use ym2149_ym_replayer::player::PlaybackController;

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No default output audio device found")?;
        let config = device.default_output_config()?;

        let sample_rate = config.sample_rate();
        let device_channels = config.channels() as usize;
        let stream_config: cpal::StreamConfig = config.into();

        // Load the YM song using the replayer crate
        let decompressed = ym2149_ym_replayer::compression::decompress_if_needed(ym_data)?;
        let (mut player, summary) =
            ym2149_ym_replayer::player::ym_player::load_song_with_rate(&decompressed, sample_rate)?;

        // Start playback state
        PlaybackController::play(&mut player)?;

        println!(
            "{} {:?}  {} {}  {} {}",
            style("FORMAT:").bold(),
            summary.format,
            style("FRAMES:").bold(),
            summary.frame_count,
            style("SAMPLES/FRAME:").bold(),
            summary.samples_per_frame
        );

        let player_mutex = Arc::new(Mutex::new(player));
        let player_cb = Arc::clone(&player_mutex);

        let err_fn = |err| eprintln!("{} {}", style("Audio stream error:").red().bold(), err);
        let sample_format = config.sample_format();

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut player = player_cb.lock().unwrap();

                    let mut temp_buf = vec![0.0f32; data.len() / device_channels];
                    player.generate_samples_into(&mut temp_buf);

                    let mut temp_idx = 0;
                    for frame in data.chunks_exact_mut(device_channels) {
                        if temp_idx < temp_buf.len() {
                            let sample_val = temp_buf[temp_idx];
                            for sample in frame.iter_mut() {
                                *sample = sample_val;
                            }
                            temp_idx += 1;
                        }
                    }
                },
                err_fn,
                None,
            )?,
            _ => return Err("Unsupported audio sample format".into()),
        };

        stream.play()?;

        let duration = player_mutex
            .lock()
            .map_err(|_| "player mutex poisoned")?
            .duration_seconds() as f64;
        println!(
            "{} {:.1}s",
            style("PLAYING SONG:").bold().green(),
            duration
        );

        let total_deciseconds = (duration * 10.0).round().max(1.0) as u64;
        let pb = time_progress_bar(total_deciseconds);

        let start = std::time::Instant::now();
        while start.elapsed().as_secs_f64() < duration {
            let elapsed = start.elapsed().as_secs_f64();
            pb.set_position(((elapsed * 10.0).round() as u64).min(total_deciseconds));
            pb.set_message(format!("{:.1}s / {:.1}s", elapsed, duration));
            std::thread::sleep(Duration::from_millis(100));
        }
        pb.finish_and_clear();

        Ok(())
    }
}
