// Animated spinner widget
// Frame-based animation for loading/thinking states
// Stateless: takes tick counter and returns current frame character

#![allow(dead_code)]

/// Spinner animation frames
/// Uses braille characters for smooth animation
const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Alternative spinner using ASCII characters (fallback for terminals without Unicode)
const SPINNER_FRAMES_ASCII: &[char] = &['|', '/', '-', '\\'];

/// Get the current spinner frame character based on tick count
/// This is a pure function — same tick always returns same character
/// Use this for deterministic, testable animations
/// 
/// # Arguments
/// * `tick` - Current tick counter (incremented each frame)
/// * `use_unicode` - Whether to use Unicode braille characters (true) or ASCII (false)
/// 
/// # Returns
/// The character to display for this frame
pub fn get_spinner_frame(tick: u64, use_unicode: bool) -> char {
    let frames = if use_unicode {
        SPINNER_FRAMES
    } else {
        SPINNER_FRAMES_ASCII
    };
    
    let index = (tick as usize) % frames.len();
    frames[index]
}

/// Get a spinner string with a label
/// Example: "⠋ Thinking..." or "| Loading..."
pub fn spinner_with_label(tick: u64, label: &str, use_unicode: bool) -> String {
    format!("{} {}", get_spinner_frame(tick, use_unicode), label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_cycles() {
        // Test that spinner cycles through all frames
        let frames = SPINNER_FRAMES;
        for i in 0..frames.len() {
            assert_eq!(get_spinner_frame(i as u64, true), frames[i]);
        }
        // Test wrap-around
        assert_eq!(get_spinner_frame(0, true), get_spinner_frame(frames.len() as u64, true));
    }

    #[test]
    fn test_ascii_spinner() {
        // Test ASCII fallback
        let frame = get_spinner_frame(0, false);
        assert!(SPINNER_FRAMES_ASCII.contains(&frame));
    }
}
