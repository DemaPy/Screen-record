//! AVFoundation device discovery. We run ffmpeg's device list and parse the
//! video section for the first "Capture screen" entry.
//!
//! Example output line (note the doubled bracket prefix):
//! `[AVFoundation indev @ 0x..] [1] Capture screen 0`

use anyhow::Result;
use regex::Regex;
use tokio::process::Command;

use crate::error::SetupError;

/// Discover the AVFoundation video device index for screen capture.
///
/// `ffmpeg -f avfoundation -list_devices true -i ""` prints devices to stderr
/// and exits non-zero (it has no real input), which is expected.
pub async fn discover_screen_index() -> Result<u32, SetupError> {
    let output = Command::new("ffmpeg")
        .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .output()
        .await
        .map_err(|e| SetupError::Other(anyhow::anyhow!("running ffmpeg list_devices: {e}")))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_screen_index(&stderr).ok_or_else(|| SetupError::NoScreenDevice(stderr.to_string()))
}

/// Parse the first `[N] Capture screen ...` index from the video-devices
/// section. Stops before the audio-devices section so we never pick an audio
/// index by accident.
fn parse_screen_index(stderr: &str) -> Option<u32> {
    let re = Regex::new(r"\[(\d+)\]\s+Capture screen").unwrap();
    for line in stderr.lines() {
        if line.contains("audio devices") {
            break;
        }
        if let Some(caps) = re.captures(line) {
            if let Ok(idx) = caps[1].parse::<u32>() {
                return Some(idx);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_output() {
        let sample = "\
[AVFoundation indev @ 0x99b064000] AVFoundation video devices:
[AVFoundation indev @ 0x99b064000] [0] FaceTime HD Camera
[AVFoundation indev @ 0x99b064000] [1] Capture screen 0
[AVFoundation indev @ 0x99b064000] AVFoundation audio devices:
[AVFoundation indev @ 0x99b064000] [0] MacBook Pro Microphone";
        assert_eq!(parse_screen_index(sample), Some(1));
    }

    #[test]
    fn ignores_audio_section() {
        // A device named like a screen in the audio section must not match.
        let sample = "\
[x] AVFoundation video devices:
[x] [0] FaceTime HD Camera
[x] AVFoundation audio devices:
[x] [3] Capture screen audio";
        assert_eq!(parse_screen_index(sample), None);
    }

    #[test]
    fn none_when_absent() {
        assert_eq!(parse_screen_index("nothing here"), None);
    }
}
