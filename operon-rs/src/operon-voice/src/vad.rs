//! Simple energy-based Voice Activity Detection (VAD).
//!
//! This module provides a lightweight silence detector that doesn't require
//! any external crate or trained model. It computes the RMS (root-mean-square)
//! energy of audio windows and classifies each window as speech or silence
//! based on a configurable threshold.
//!
//! ## Design Rationale
//!
//! We deliberately avoid pulling in a heavier VAD library (like webrtc-vad)
//! because:
//! 1. Our use case is simple — detect prolonged silence to auto-stop recording.
//! 2. Whisper itself is very robust to leading/trailing silence, so false
//!    positives (classifying silence as speech) just mean a slightly longer
//!    recording, not a transcription failure.
//! 3. Fewer dependencies = faster builds and smaller binary.
//!
//! ## How It Works
//!
//! The `SilenceDetector` tracks how many consecutive seconds of audio have
//! been below the energy threshold. When this duration exceeds the configured
//! `silence_threshold_secs`, it reports that the speaker has stopped talking.

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// The sample rate that the VAD expects. This matches the 16 kHz rate used
/// by capture.rs and whisper.cpp, so no additional resampling is needed.
pub const VAD_SAMPLE_RATE: u32 = 16_000;

/// Size of each analysis window in samples. At 16 kHz, 480 samples = 30ms.
/// This is the standard frame size for speech processing — small enough to
/// be responsive, large enough to give a stable energy estimate.
const WINDOW_SIZE: usize = 480;

/// Default RMS energy threshold below which audio is classified as silence.
///
/// This value was tuned empirically for typical desktop microphones. It's
/// intentionally conservative (low) to avoid cutting off quiet speakers.
/// A value of ~0.01 catches most ambient room noise as silence while still
/// detecting normal conversational speech.
const DEFAULT_ENERGY_THRESHOLD: f32 = 0.01;

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the RMS (root-mean-square) energy of an audio buffer.
///
/// RMS is a standard measure of signal energy / loudness. A higher RMS value
/// means louder audio. Returns 0.0 for empty buffers.
///
/// ```
/// let silence = vec![0.0_f32; 100];
/// assert_eq!(operon_voice::vad::rms_energy(&silence), 0.0);
///
/// let tone = vec![0.5_f32; 100];
/// assert!((operon_voice::vad::rms_energy(&tone) - 0.5).abs() < 0.001);
/// ```
pub fn rms_energy(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    // Sum of squares, then divide by count, then square root
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Check if a single buffer of audio samples contains speech.
///
/// Returns `true` if the RMS energy is above the threshold (i.e. someone is
/// probably talking), `false` if it's likely silence or ambient noise.
pub fn is_speech(samples: &[f32], threshold: f32) -> bool {
    rms_energy(samples) > threshold
}

// ─────────────────────────────────────────────────────────────────────────────
// SilenceDetector — stateful silence tracking
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks consecutive silence duration to detect end-of-speech.
///
/// Feed audio samples into the detector via `feed()`. After each feed, call
/// `is_silent()` to check if enough consecutive silence has elapsed to
/// consider the speaker done.
///
/// ## Example
///
/// ```rust
/// use operon_voice::vad::SilenceDetector;
///
/// let mut detector = SilenceDetector::new(16000, 1.5);
///
/// // Feed 2 seconds of silence (32000 samples of zeros at 16kHz)
/// let silence = vec![0.0_f32; 32000];
/// detector.feed(&silence);
///
/// assert!(detector.is_silent()); // 2s of silence > 1.5s threshold
/// ```
pub struct SilenceDetector {
    /// The audio sample rate (needed to convert sample counts to seconds).
    sample_rate: u32,

    /// How many consecutive seconds of silence are required before we declare
    /// "the speaker has stopped talking".
    silence_threshold_secs: f64,

    /// RMS energy level below which a window is classified as silence.
    energy_threshold: f32,

    /// Count of consecutive silent samples seen so far. Reset to 0 whenever
    /// a speech window is detected.
    consecutive_silent_samples: usize,

    /// Leftover samples from previous `feed()` calls that didn't fill a
    /// complete analysis window. Carried over to the next feed.
    leftover: Vec<f32>,
}

impl SilenceDetector {
    /// Create a new silence detector.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` — The audio sample rate in Hz (typically 16000).
    /// * `silence_threshold_secs` — How many seconds of continuous silence
    ///   must occur before `is_silent()` returns `true`.
    pub fn new(sample_rate: u32, silence_threshold_secs: f64) -> Self {
        Self {
            sample_rate,
            silence_threshold_secs,
            energy_threshold: DEFAULT_ENERGY_THRESHOLD,
            consecutive_silent_samples: 0,
            leftover: Vec::with_capacity(WINDOW_SIZE),
        }
    }

    /// Feed a chunk of audio samples into the detector.
    ///
    /// The samples are split into fixed-size windows of `WINDOW_SIZE` samples.
    /// Each window is independently classified as speech or silence. Any
    /// leftover samples that don't fill a complete window are carried over to
    /// the next `feed()` call.
    pub fn feed(&mut self, samples: &[f32]) {
        // Prepend any leftover samples from the previous feed
        self.leftover.extend_from_slice(samples);

        // Process complete windows
        let mut offset = 0;
        while offset + WINDOW_SIZE <= self.leftover.len() {
            let window = &self.leftover[offset..offset + WINDOW_SIZE];

            if is_speech(window, self.energy_threshold) {
                // Speech detected — reset the consecutive silence counter
                self.consecutive_silent_samples = 0;
            } else {
                // Silence — accumulate the window's sample count
                self.consecutive_silent_samples += WINDOW_SIZE;
            }

            offset += WINDOW_SIZE;
        }

        // Keep any remaining samples that didn't fill a complete window
        // for the next feed() call
        if offset < self.leftover.len() {
            let remaining = self.leftover[offset..].to_vec();
            self.leftover = remaining;
        } else {
            self.leftover.clear();
        }
    }

    /// Check if the speaker has been silent long enough to consider them done.
    ///
    /// Returns `true` if consecutive silence duration ≥ the configured threshold.
    pub fn is_silent(&self) -> bool {
        let silent_seconds =
            self.consecutive_silent_samples as f64 / self.sample_rate as f64;
        silent_seconds >= self.silence_threshold_secs
    }

    /// Reset the detector state. Call this when starting a new recording session.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.consecutive_silent_samples = 0;
        self.leftover.clear();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rms_energy_silence() {
        // Pure silence should have zero energy
        let silence = vec![0.0_f32; 1000];
        assert_eq!(rms_energy(&silence), 0.0);
    }

    #[test]
    fn test_rms_energy_constant_signal() {
        // A constant signal of 0.5 should have RMS of 0.5
        let signal = vec![0.5_f32; 1000];
        let energy = rms_energy(&signal);
        assert!((energy - 0.5).abs() < 0.001, "Expected ~0.5, got {}", energy);
    }

    #[test]
    fn test_rms_energy_empty_buffer() {
        // Empty buffer should return 0
        assert_eq!(rms_energy(&[]), 0.0);
    }

    #[test]
    fn test_is_speech_detects_loud_signal() {
        // A loud signal should be detected as speech
        let loud = vec![0.5_f32; 480];
        assert!(is_speech(&loud, DEFAULT_ENERGY_THRESHOLD));
    }

    #[test]
    fn test_is_speech_rejects_silence() {
        // Pure silence should not be detected as speech
        let silence = vec![0.0_f32; 480];
        assert!(!is_speech(&silence, DEFAULT_ENERGY_THRESHOLD));
    }

    #[test]
    fn test_silence_detector_detects_prolonged_silence() {
        let mut detector = SilenceDetector::new(16_000, 1.0);

        // Feed 2 seconds of silence (32000 samples at 16kHz)
        let silence = vec![0.0_f32; 32_000];
        detector.feed(&silence);

        // 2 seconds > 1 second threshold → should be silent
        assert!(detector.is_silent());
    }

    #[test]
    fn test_silence_detector_resets_on_speech() {
        let mut detector = SilenceDetector::new(16_000, 1.0);

        // Feed 1.5 seconds of silence
        let silence = vec![0.0_f32; 24_000];
        detector.feed(&silence);
        assert!(detector.is_silent());

        // Feed a loud burst — should reset the silence counter
        let speech = vec![0.5_f32; 480];
        detector.feed(&speech);
        assert!(!detector.is_silent());
    }

    #[test]
    fn test_silence_detector_not_silent_for_short_pause() {
        let mut detector = SilenceDetector::new(16_000, 1.5);

        // Feed only 0.5 seconds of silence — below the 1.5s threshold
        let silence = vec![0.0_f32; 8_000];
        detector.feed(&silence);

        assert!(!detector.is_silent());
    }
}
