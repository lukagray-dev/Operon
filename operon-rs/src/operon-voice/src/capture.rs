//! Microphone audio capture via cpal.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

pub struct CaptureHandle {
    _stream: cpal::Stream,
}

pub fn start_capture() -> Result<(CaptureHandle, Arc<Mutex<Vec<f32>>>)> {
    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .context("No default audio input device found. Is a microphone connected?")?;

    eprintln!("[operon-voice][capture] Using default audio input device");

    let config = device
        .default_input_config()
        .context("Failed to get default input config from audio device")?;

    let device_sample_rate: u32 = config.sample_rate();
    let device_channels = config.channels() as usize;

    eprintln!(
        "[operon-voice][capture] Device config: {} Hz, {} channels, format: {:?}",
        device_sample_rate,
        device_channels,
        config.sample_format()
    );

    let audio_buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let buffer_clone = Arc::clone(&audio_buffer);

    let resample_ratio = device_sample_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let resample_position = Arc::new(Mutex::new(0.0_f64));

    let err_fn = |err| {
        eprintln!("[operon-voice][capture] Audio stream error: {}", err);
    };

    let stream_config: cpal::StreamConfig = config.into();

    let stream = device
        .build_input_stream(
            stream_config,
            move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                process_audio_callback(
                    data,
                    device_channels,
                    resample_ratio,
                    &resample_position,
                    &buffer_clone,
                );
            },
            err_fn,
            None,
        )
        .context("Failed to build audio input stream")?;

    stream.play().context("Failed to start audio input stream")?;

    eprintln!("[operon-voice][capture] Audio capture started");

    Ok((CaptureHandle { _stream: stream }, audio_buffer))
}

fn process_audio_callback(
    data: &[f32],
    channels: usize,
    resample_ratio: f64,
    resample_position: &Arc<Mutex<f64>>,
    output_buffer: &Arc<Mutex<Vec<f32>>>,
) {
    let num_frames = data.len() / channels;
    let mut mono_samples = Vec::with_capacity(num_frames);

    for frame_idx in 0..num_frames {
        let mut sum = 0.0_f32;
        for ch in 0..channels {
            sum += data[frame_idx * channels + ch];
        }
        mono_samples.push(sum / channels as f32);
    }

    let mut pos = resample_position.lock().unwrap();
    let mut resampled = Vec::new();

    while (*pos as usize) < mono_samples.len() {
        let idx = *pos as usize;
        resampled.push(mono_samples[idx]);
        *pos += resample_ratio;
    }

    *pos -= num_frames as f64;
    drop(pos);

    if !resampled.is_empty() {
        let mut buf = output_buffer.lock().unwrap();
        buf.extend_from_slice(&resampled);
    }
}
