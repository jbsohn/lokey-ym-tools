use crate::sequence::{SfxSequence, YmChannel, YmSequence};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ym2149::{Ym2149, Ym2149Backend};

pub struct AudioPlayer;

impl AudioPlayer {
    pub fn play_sfx(sequence: &SfxSequence) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No default output audio device found")?;
        let config = device.default_output_config()?;

        let sample_rate = config.sample_rate();
        let channels = config.channels() as usize;
        let stream_config: cpal::StreamConfig = config.into();

        let master_clock = sequence.source_clock;
        let chip = Ym2149::with_clocks(master_clock, sample_rate);

        let hz = sequence.source_hz;
        let samples_per_frame = (sample_rate as f64 / hz as f64) as usize;

        let frames = sequence.frames.clone();
        if frames.is_empty() {
            println!("Sequence contains no frames to play.");
            return Ok(());
        }

        let current_frame_idx = Arc::new(Mutex::new(0usize));
        let sample_in_frame = Arc::new(Mutex::new(0usize));
        let mixer = Arc::new(Mutex::new(0x3F)); // Default: all tones & noise muted

        let chip_mutex = Arc::new(Mutex::new(chip));

        let channel = sequence
            .preferred_channels
            .as_ref()
            .and_then(|c| c.first().copied())
            .unwrap_or(YmChannel::A);

        // Apply initial frame registers
        {
            let mut chip_guard = chip_mutex.lock().unwrap();
            let mut mixer_guard = mixer.lock().unwrap();
            frames[0].apply_to_chip(&mut *chip_guard, &mut *mixer_guard, channel);
        }

        let total_frames = frames.len();
        let finished = Arc::new(Mutex::new(false));

        let current_frame_idx_cb = Arc::clone(&current_frame_idx);
        let sample_in_frame_cb = Arc::clone(&sample_in_frame);
        let mixer_cb = Arc::clone(&mixer);
        let chip_cb = Arc::clone(&chip_mutex);
        let finished_cb = Arc::clone(&finished);
        let frames_cb = frames;

        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let sample_format = config.sample_format();

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut chip = chip_cb.lock().unwrap();
                    let mut frame_idx = current_frame_idx_cb.lock().unwrap();
                    let mut sample_count = sample_in_frame_cb.lock().unwrap();
                    let mut mix = mixer_cb.lock().unwrap();
                    let mut is_done = finished_cb.lock().unwrap();

                    if *is_done {
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                        return;
                    }

                    let mut i = 0;
                    while i < data.len() {
                        let sample_val = chip.get_sample();
                        chip.clock();

                        for c in 0..channels {
                            if i + c < data.len() {
                                data[i + c] = sample_val;
                            }
                        }
                        i += channels;

                        *sample_count += 1;
                        if *sample_count >= samples_per_frame {
                            *sample_count = 0;
                            *frame_idx += 1;

                            if *frame_idx >= total_frames {
                                *is_done = true;
                            } else {
                                frames_cb[*frame_idx].apply_to_chip(&mut *chip, &mut *mix, channel);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )?,
            _ => return Err("Unsupported audio sample format".into()),
        };

        stream.play()?;

        println!(
            "PLAYING SOUND EFFECT: '{}' ({} frames @ {} Hz on channel {:?})",
            sequence.name, total_frames, hz, channel
        );

        let total_duration_secs = (total_frames as f64) / (hz as f64);
        std::thread::sleep(Duration::from_secs_f64(total_duration_secs + 0.1));

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
        let stream_config: cpal::StreamConfig = config.into();

        let master_clock = sequence.timing.master_clock_hz;
        let chip = Ym2149::with_clocks(master_clock, sample_rate);

        let hz = sequence.timing.frame_rate.hz_value();
        let samples_per_frame = (sample_rate as f64 / hz as f64) as usize;

        let frames = sequence.frames.clone();
        if frames.is_empty() {
            println!("Sequence contains no frames to play.");
            return Ok(());
        }

        let current_frame_idx = Arc::new(Mutex::new(0usize));
        let sample_in_frame = Arc::new(Mutex::new(0usize));
        let mixer = Arc::new(Mutex::new(0x3F)); // Default: all tones & noise muted

        let chip_mutex = Arc::new(Mutex::new(chip));

        // Apply initial frame registers
        {
            let mut chip_guard = chip_mutex.lock().unwrap();
            let mut mixer_guard = mixer.lock().unwrap();
            frames[0].apply_to_chip(&mut *chip_guard, &mut *mixer_guard);
        }

        let total_frames = frames.len();
        let loop_start_val = sequence.loop_start;
        let finished = Arc::new(Mutex::new(false));

        let current_frame_idx_cb = Arc::clone(&current_frame_idx);
        let sample_in_frame_cb = Arc::clone(&sample_in_frame);
        let mixer_cb = Arc::clone(&mixer);
        let chip_cb = Arc::clone(&chip_mutex);
        let finished_cb = Arc::clone(&finished);
        let frames_cb = frames;

        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let sample_format = config.sample_format();

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut chip = chip_cb.lock().unwrap();
                    let mut frame_idx = current_frame_idx_cb.lock().unwrap();
                    let mut sample_count = sample_in_frame_cb.lock().unwrap();
                    let mut mix = mixer_cb.lock().unwrap();
                    let mut is_done = finished_cb.lock().unwrap();

                    if *is_done {
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                        return;
                    }

                    let mut i = 0;
                    while i < data.len() {
                        let sample_val = chip.get_sample();
                        chip.clock();

                        for c in 0..channels {
                            if i + c < data.len() {
                                data[i + c] = sample_val;
                            }
                        }
                        i += channels;

                        *sample_count += 1;
                        if *sample_count >= samples_per_frame {
                            *sample_count = 0;
                            *frame_idx += 1;

                            if *frame_idx >= total_frames {
                                if let Some(l_start) = loop_start_val {
                                    *frame_idx = l_start;
                                    frames_cb[*frame_idx].apply_to_chip(&mut *chip, &mut *mix);
                                } else {
                                    *is_done = true;
                                }
                            } else {
                                frames_cb[*frame_idx].apply_to_chip(&mut *chip, &mut *mix);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )?,
            _ => return Err("Unsupported audio sample format".into()),
        };

        stream.play()?;

        println!(
            "PLAYING SONG: '{}' ({} frames @ {} Hz)",
            sequence.name, total_frames, hz
        );

        let total_duration_secs = (total_frames as f64) / (hz as f64);
        let is_looping = loop_start_val.is_some();
        let start = std::time::Instant::now();

        while (is_looping || start.elapsed().as_secs_f64() < total_duration_secs)
            && !*finished.lock().unwrap()
        {
            let current_frame = *current_frame_idx.lock().unwrap();
            let current_secs = (current_frame as f64) / (hz as f64);
            print!(
                "\rProgress: Frame {:5}/{} | Time: {:6.1}s / {:.1}s{}",
                current_frame,
                total_frames,
                current_secs,
                total_duration_secs,
                if is_looping { " (Looping)" } else { "" }
            );
            use std::io::Write;
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }
        println!();

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
            "SONG FORMAT: {:?}\nTOTAL FRAMES: {}\nSAMPLES PER FRAME: {}",
            summary.format, summary.frame_count, summary.samples_per_frame
        );

        let player_mutex = Arc::new(Mutex::new(player));
        let player_cb = Arc::clone(&player_mutex);

        let err_fn = |err| eprintln!("Audio stream error: {}", err);
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

        let duration = player_mutex.lock().unwrap().duration_seconds() as f64;
        println!("Playing... Duration: {:.1} seconds", duration);

        let start = std::time::Instant::now();
        while start.elapsed().as_secs_f64() < duration {
            let elapsed = start.elapsed().as_secs_f64();
            print!("\rProgress: Time: {:6.1}s / {:.1}s", elapsed, duration);
            use std::io::Write;
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }
        println!();

        Ok(())
    }
}
