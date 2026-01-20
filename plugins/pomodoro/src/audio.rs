//! Audio playback for pomodoro alerts
//!
//! Uses rodio to play generated tones for break start/end alerts.

use crate::config::{AudioSettings, PomoConfig};
use rodio::source::{SineWave, Zero};
use rodio::{OutputStreamBuilder, Sink, Source};
use std::time::Duration;

/// Play an alert sound (ascending tones for break start)
pub fn play_break_start_alert() {
    let config = PomoConfig::default().audio;
    std::thread::spawn(move || {
        if let Err(e) = play_ascending_tones(&config) {
            eprintln!("Warning: Failed to play break start sound: {}", e);
        }
    });
}

/// Play an alert sound (descending tones for break end)
pub fn play_break_end_alert() {
    let config = PomoConfig::default().audio;
    std::thread::spawn(move || {
        if let Err(e) = play_descending_tones(&config) {
            eprintln!("Warning: Failed to play break end sound: {}", e);
        }
    });
}

/// Play ascending tones (break start - cheerful, relaxing)
fn play_ascending_tones(config: &AudioSettings) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _stream = OutputStreamBuilder::open_default_stream()?;
    let sink = Sink::connect_new(_stream.mixer());

    let duration = Duration::from_millis(config.note_duration_ms);
    let pause = Duration::from_millis(config.note_pause_ms);

    for freq in config.break_start_frequencies {
        let source = SineWave::new(freq)
            .take_duration(duration)
            .amplify(config.volume);
        sink.append(source);

        // Small pause between notes
        let silence = Zero::new(2, config.sample_rate).take_duration(pause);
        sink.append(silence);
    }

    // Final chord
    let final_freq = config.break_start_frequencies[2]; // G5
    let final_source = SineWave::new(final_freq)
        .take_duration(Duration::from_millis(config.final_note_duration_ms))
        .amplify(config.final_volume);
    sink.append(final_source);

    sink.sleep_until_end();
    Ok(())
}

/// Play descending tones (break end - gentle reminder to get back to work)
fn play_descending_tones(config: &AudioSettings) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _stream = OutputStreamBuilder::open_default_stream()?;
    let sink = Sink::connect_new(_stream.mixer());

    let duration = Duration::from_millis(config.note_duration_ms);
    let pause = Duration::from_millis(config.note_pause_ms);

    for freq in config.break_end_frequencies {
        let source = SineWave::new(freq)
            .take_duration(duration)
            .amplify(config.volume);
        sink.append(source);

        let silence = Zero::new(2, config.sample_rate).take_duration(pause);
        sink.append(silence);
    }

    // Double beep at the end
    for _ in 0..2 {
        let beep = SineWave::new(config.beep_frequency)
            .take_duration(Duration::from_millis(config.beep_duration_ms))
            .amplify(config.final_volume);
        sink.append(beep);

        let silence = Zero::new(2, config.sample_rate)
            .take_duration(Duration::from_millis(config.beep_pause_ms));
        sink.append(silence);
    }

    sink.sleep_until_end();
    Ok(())
}

/// Simple beep for testing
#[allow(dead_code)]
pub fn play_test_beep() {
    let config = PomoConfig::default().audio;
    std::thread::spawn(move || {
        if let Err(e) = play_single_beep(&config) {
            eprintln!("Warning: Failed to play test beep: {}", e);
        }
    });
}

fn play_single_beep(config: &AudioSettings) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _stream = OutputStreamBuilder::open_default_stream()?;
    let sink = Sink::connect_new(_stream.mixer());

    let source = SineWave::new(config.test_beep_frequency)
        .take_duration(Duration::from_millis(config.note_duration_ms + 100))
        .amplify(config.volume);
    sink.append(source);

    sink.sleep_until_end();
    Ok(())
}
