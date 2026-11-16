use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

// 30fps 下缓存 500ms 的解码帧
const CAPACITY: usize = (super::FPS * 0.5) as usize;

#[derive(Default)]
struct State {
    paused: bool,
    stopped: bool,
    video_ended: bool,
    #[cfg(target_os = "linux")]
    audio_ended: bool,
    #[cfg(not(target_os = "linux"))]
    next: Option<Instant>,
    pending: VecDeque<(u64, super::VideoFrame)>,
    latest: Option<(u64, super::VideoFrame)>,
    audio_sample: Option<(f64, Instant)>,
    position: f64,
}

impl State {
    fn position(&mut self, now: Instant) -> f64 {
        if !self.paused {
            if let Some((time, sampled)) = self.audio_sample {
                // 插值覆盖两次音频采样之间的时间，断流时最多推进 100ms
                let elapsed = now.duration_since(sampled).as_secs_f64();
                #[cfg(target_os = "linux")]
                let elapsed = if self.audio_ended {
                    elapsed
                } else {
                    elapsed.min(0.1)
                };
                self.position = self.position.max(time + elapsed);
            }
        }
        self.position
    }
}

#[derive(Default)]
pub(super) struct Frames {
    state: Mutex<State>,
    changed: Condvar,
    live: bool,
}

impl Frames {
    pub fn new(live: bool) -> Self {
        Self {
            live,
            ..Self::default()
        }
    }

    pub fn pause(&self, paused: bool) {
        let mut state = self.state.lock().unwrap();
        state.position(Instant::now());
        state.paused = paused;
        state.audio_sample = None;
        #[cfg(target_os = "linux")]
        if state.audio_ended && !paused {
            state.audio_sample = Some((state.position, Instant::now()));
        }
        #[cfg(not(target_os = "linux"))]
        {
            state.next = None;
        }
        self.changed.notify_all();
    }

    #[cfg(target_os = "linux")]
    pub fn update_audio_clock(&self, time: f64) {
        if !time.is_finite() || time <= 0.0 {
            return;
        }
        let mut state = self.state.lock().unwrap();
        if !state.paused {
            state.audio_sample = Some((time, Instant::now()));
            self.changed.notify_all();
        }
    }

    pub fn position(&self, _index: u64) -> f64 {
        let mut state = self.state.lock().unwrap();
        #[cfg(not(target_os = "linux"))]
        if !state.paused {
            state.position = state
                .position
                .max(_index.saturating_sub(1) as f64 / super::FPS);
        }
        state.position(Instant::now())
    }

    pub fn stop(&self) {
        self.state.lock().unwrap().stopped = true;
        self.changed.notify_all();
    }

    pub fn finish_video(&self) {
        self.state.lock().unwrap().video_ended = true;
        self.changed.notify_all();
    }

    #[cfg(target_os = "linux")]
    pub fn finish_audio(&self) {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        let position = state.position(now);
        // 音频排空后，剩余视频帧沿用当前时钟播放
        state.audio_sample = Some((position, now));
        state.audio_ended = true;
        self.changed.notify_all();
    }

    pub fn publish(&self, index: u64, pixels: super::VideoFrame) -> bool {
        let mut state = self.state.lock().unwrap();
        while !state.stopped && state.pending.len() >= CAPACITY {
            if self.live && !state.paused {
                let position = state.position(Instant::now());
                let (next, _) = self
                    .changed
                    .wait_timeout_while(state, Duration::from_millis(250), |state| {
                        !state.stopped && !state.paused && state.pending.len() >= CAPACITY
                    })
                    .unwrap();
                state = next;
                // 音频时钟停滞时释放一帧，让解码器继续输出音频
                if !state.paused
                    && state.pending.len() >= CAPACITY
                    && state.position(Instant::now()) <= position
                {
                    state.pending.pop_front();
                }
            } else {
                state = self.changed.wait(state).unwrap();
            }
        }
        if state.stopped {
            return false;
        }
        state.pending.push_back((index, pixels));
        self.changed.notify_all();
        true
    }

    pub fn present(&self) -> Option<bool> {
        let mut state = self.state.lock().unwrap();
        loop {
            if state.stopped {
                return None;
            }
            let Some(&(index, _)) = state.pending.front() else {
                #[cfg(target_os = "linux")]
                let ended = state.video_ended && state.audio_ended;
                #[cfg(not(target_os = "linux"))]
                let ended = state.video_ended;
                if ended {
                    state.position(Instant::now());
                    state.audio_sample = None;
                    return None;
                }
                state = self.changed.wait(state).unwrap();
                continue;
            };
            // 首帧可在加载中暂停时作为预览
            if state.paused && index > 1 {
                state = self.changed.wait(state).unwrap();
                continue;
            }
            #[cfg(target_os = "linux")]
            if !state.paused {
                if state.audio_sample.is_none() {
                    state = self.changed.wait(state).unwrap();
                    continue;
                }
                let ahead = (index - 1) as f64 / super::FPS - state.position(Instant::now());
                if ahead > 0.002 {
                    let wait = Duration::from_secs_f64(ahead.min(0.05));
                    state = self.changed.wait_timeout(state, wait).unwrap().0;
                    continue;
                }
                if ahead < -1.0 / super::FPS && state.pending.len() > 1 {
                    state.pending.pop_front();
                    self.changed.notify_all();
                    continue;
                }
                break;
            }
            #[cfg(not(target_os = "linux"))]
            if let Some(next) = state.next {
                if let Some(wait) = next.checked_duration_since(Instant::now()) {
                    state = self.changed.wait_timeout(state, wait).unwrap().0;
                    continue;
                }
            }
            break;
        }
        #[cfg(not(target_os = "linux"))]
        {
            let interval = Duration::from_secs_f64(1.0 / super::FPS);
            state.next = Some(state.next.unwrap_or_else(Instant::now) + interval);
        }
        let notify = state.latest.is_none();
        state.latest = state.pending.pop_front();
        self.changed.notify_all();
        Some(notify)
    }

    pub fn take(&self) -> Option<(u64, super::VideoFrame)> {
        self.state.lock().unwrap().latest.take()
    }
}

#[cfg(test)]
mod tests;
