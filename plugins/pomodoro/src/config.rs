//! Centralized configuration for the Pomodoro plugin
//!
//! All magic numbers and configurable values are consolidated here with sensible defaults.

use serde::{Deserialize, Serialize};

/// Main configuration structure for the Pomodoro plugin
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PomoConfig {
    /// Timer-related settings
    pub timer: TimerSettings,
    /// Audio alert settings
    pub audio: AudioSettings,
    /// UI/window settings
    pub ui: UiSettings,
    /// Timing/delay settings
    pub timing: TimingSettings,
}

/// Timer-related settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerSettings {
    /// Default focus duration in minutes
    pub default_focus_minutes: u64,
    /// Default break duration in minutes
    pub default_break_minutes: u64,
    /// Long break duration in minutes
    pub long_break_minutes: u64,
    /// Number of pomodoros before a long break
    pub pomodoros_before_long_break: u32,
    /// Default number of cycles
    pub default_cycles: u32,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            default_focus_minutes: 25,
            default_break_minutes: 5,
            long_break_minutes: 15,
            pomodoros_before_long_break: 4,
            default_cycles: 4,
        }
    }
}

/// Audio alert settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Master volume (0.0 - 1.0)
    pub volume: f32,
    /// Volume for final/ending tones
    pub final_volume: f32,
    /// Frequencies for break start alert (ascending: C5, E5, G5)
    pub break_start_frequencies: [f32; 3],
    /// Frequencies for break end alert (descending: G5, E5, C5)
    pub break_end_frequencies: [f32; 3],
    /// Frequency for ending double beep
    pub beep_frequency: f32,
    /// Frequency for test beep
    pub test_beep_frequency: f32,
    /// Duration of each note in milliseconds
    pub note_duration_ms: u64,
    /// Duration of final note in milliseconds
    pub final_note_duration_ms: u64,
    /// Duration of beep in milliseconds
    pub beep_duration_ms: u64,
    /// Pause between notes in milliseconds
    pub note_pause_ms: u64,
    /// Pause between beeps in milliseconds
    pub beep_pause_ms: u64,
    /// Sample rate for silence generation
    pub sample_rate: u32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            volume: 0.3,
            final_volume: 0.25,
            // C5, E5, G5 - major chord
            break_start_frequencies: [523.25, 659.25, 783.99],
            // G5, E5, C5 - descending
            break_end_frequencies: [783.99, 659.25, 523.25],
            beep_frequency: 880.0,      // A5
            test_beep_frequency: 440.0, // A4
            note_duration_ms: 200,
            final_note_duration_ms: 400,
            beep_duration_ms: 150,
            note_pause_ms: 50,
            beep_pause_ms: 100,
            sample_rate: 44100,
        }
    }
}

/// UI/window settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    /// Window width in pixels
    pub window_width: f32,
    /// Window height in pixels
    pub window_height: f32,
    /// Main countdown timer font size
    pub countdown_font_size: f32,
    /// Secondary message font size
    pub message_font_size: f32,
    /// Break type label font size
    pub label_font_size: f32,
    /// Timer text color (RGB)
    pub timer_color: [u8; 3],
    /// Auto-close delay after break ends (seconds)
    pub auto_close_delay_secs: u64,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            window_width: 300.0,
            window_height: 200.0,
            countdown_font_size: 48.0,
            message_font_size: 18.0,
            label_font_size: 14.0,
            timer_color: [100, 200, 100], // Light green
            auto_close_delay_secs: 3,
        }
    }
}

/// Timing/delay settings for daemon operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingSettings {
    /// Time to wait for daemon to start up (milliseconds)
    pub daemon_startup_wait_ms: u64,
    /// Tick interval for timer checks (seconds)
    pub tick_interval_secs: u64,
    /// IPC message buffer size
    pub ipc_buffer_size: usize,
}

impl Default for TimingSettings {
    fn default() -> Self {
        Self {
            daemon_startup_wait_ms: 500,
            tick_interval_secs: 1,
            ipc_buffer_size: 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PomoConfig::default();
        assert_eq!(config.timer.long_break_minutes, 15);
        assert_eq!(config.timer.pomodoros_before_long_break, 4);
        assert!((config.audio.volume - 0.3).abs() < f32::EPSILON);
        assert_eq!(config.ui.window_width, 300.0);
        assert_eq!(config.timing.daemon_startup_wait_ms, 500);
    }

    #[test]
    fn test_config_serialization() {
        let config = PomoConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: PomoConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.timer.long_break_minutes, parsed.timer.long_break_minutes);
    }
}
