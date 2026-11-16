use super::*;

fn pixels() -> crate::player::VideoFrame {
    crate::player::VideoFrame::new(1, 1)
}

#[test]
fn paused_clock_does_not_advance_with_audio_sample() {
    let now = Instant::now();
    let mut state = State {
        paused: true,
        position: 2.0,
        audio_sample: Some((10.0, now)),
        ..Default::default()
    };
    assert_eq!(state.position(now + Duration::from_secs(1)), 2.0);
}

#[test]
fn clock_interpolates_audio_samples_without_moving_backwards() {
    let now = Instant::now();
    let mut state = State {
        position: 2.0,
        audio_sample: Some((3.0, now)),
        ..Default::default()
    };
    assert!((state.position(now + Duration::from_millis(50)) - 3.05).abs() < 1e-9);
    state.audio_sample = Some((1.0, now));
    assert!((state.position(now + Duration::from_millis(60)) - 3.05).abs() < 1e-9);
}

#[test]
fn pausing_clears_audio_sample_and_preserves_position() {
    let frames = Frames::new(false);
    frames.state.lock().unwrap().position = 3.0;
    frames.pause(true);
    assert_eq!(frames.position(900), 3.0);
    assert!(frames.state.lock().unwrap().audio_sample.is_none());
}

#[test]
fn first_frame_is_available_as_preview_while_paused() {
    let frames = Frames::new(false);
    frames.pause(true);
    assert!(frames.publish(1, pixels()));
    assert_eq!(frames.present(), Some(true));
    assert_eq!(frames.take().unwrap().0, 1);
    assert!(frames.take().is_none());
}

#[test]
fn stop_rejects_new_frames_and_ends_presentation() {
    let frames = Frames::new(false);
    frames.stop();
    assert!(!frames.publish(1, pixels()));
    assert_eq!(frames.present(), None);
}

#[test]
fn drained_stream_ends_after_required_tracks_finish() {
    let frames = Frames::new(false);
    frames.finish_video();
    #[cfg(target_os = "linux")]
    frames.finish_audio();
    assert_eq!(frames.present(), None);
}

#[test]
fn stop_releases_publisher_waiting_on_full_queue() {
    use std::sync::{mpsc, Arc};
    let frames = Arc::new(Frames::new(false));
    for index in 1..=CAPACITY {
        assert!(frames.publish(index as u64, pixels()));
    }
    let worker_frames = frames.clone();
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        tx.send(worker_frames.publish(99, pixels())).unwrap();
    });
    let waiting = rx.recv_timeout(Duration::from_millis(30));
    frames.stop();
    let result = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.join().unwrap();
    assert_eq!(waiting, Err(mpsc::RecvTimeoutError::Timeout));
    assert!(!result);
}

#[cfg(target_os = "linux")]
#[test]
fn active_audio_clock_caps_interpolation_until_audio_finishes() {
    let now = Instant::now();
    let mut state = State {
        audio_sample: Some((2.0, now)),
        ..Default::default()
    };
    assert_eq!(state.position(now + Duration::from_secs(1)), 2.1);
    state.audio_ended = true;
    assert_eq!(state.position(now + Duration::from_secs(1)), 3.0);
}

#[cfg(target_os = "linux")]
#[test]
fn invalid_audio_clock_samples_are_ignored() {
    let frames = Frames::new(false);
    for time in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        frames.update_audio_clock(time);
        assert!(frames.state.lock().unwrap().audio_sample.is_none());
    }
    frames.update_audio_clock(1.0);
    assert!(frames.state.lock().unwrap().audio_sample.is_some());
    frames.pause(true);
    frames.update_audio_clock(2.0);
    assert!(frames.state.lock().unwrap().audio_sample.is_none());
}
