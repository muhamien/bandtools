//! Audio engine: live guitar through the rig, track playback, metronome,
//! tab playback (plucked-string synth), recording and the tuner feed.

use crate::dsp::{ToneChain, ToneSettings};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct Track {
    pub name: String,
    pub samples: Arc<Vec<f32>>,
    pub volume: f32,
    pub mute: bool,
    pub solo: bool,
    pub color: [u8; 3],
    pub peaks: Vec<f32>,
}

impl Track {
    pub fn new(name: &str, samples: Vec<f32>, color: [u8; 3]) -> Self {
        let chunk = (samples.len() / 600).max(1);
        let peaks = samples.chunks(chunk).map(|c| c.iter().fold(0.0f32, |m, v| m.max(v.abs()))).collect();
        Self { name: name.into(), samples: Arc::new(samples), volume: 0.8, mute: false, solo: false, color, peaks }
    }
}

struct Voice {
    buf: Vec<f32>,
    idx: usize,
    left: usize,
    amp: f32,
}

pub struct TabEvent {
    pub at: u64,
    pub midi: i32,
    pub len: u64,
}

pub struct Engine {
    pub sr: f32,
    pub playing: bool,
    pub pos: u64,
    pub tempo: f32,
    pub beats_per_bar: u32,
    pub metronome: bool,
    pub master: f32,
    pub tracks: Vec<Track>,
    pub monitor: bool,
    pub monitor_level: f32,
    pub recording: bool,
    pub rec_buf: Vec<f32>,
    pub tab_events: Vec<TabEvent>,
    pub tab_volume: f32,
    next_tab: usize,
    voices: Vec<Voice>,
    pub chain: ToneChain,
    pub input: Arc<Mutex<VecDeque<f32>>>,
    pub tuner_buf: Arc<Mutex<VecDeque<f32>>>,
    pub level: f32,
    seed: u32,
}

impl Engine {
    pub fn new(sr: f32, tone: &ToneSettings) -> Self {
        Self {
            sr,
            playing: false,
            pos: 0,
            tempo: 120.0,
            beats_per_bar: 4,
            metronome: false,
            master: 0.8,
            tracks: vec![],
            monitor: false,
            monitor_level: 0.8,
            recording: false,
            rec_buf: vec![],
            tab_events: vec![],
            tab_volume: 0.7,
            next_tab: 0,
            voices: vec![],
            chain: ToneChain::new(sr, tone),
            input: Arc::new(Mutex::new(VecDeque::new())),
            tuner_buf: Arc::new(Mutex::new(VecDeque::new())),
            level: 0.0,
            seed: 12345,
        }
    }

    pub fn seek(&mut self, sample: u64) {
        self.pos = sample;
        self.next_tab = self.tab_events.iter().position(|e| e.at >= sample).unwrap_or(self.tab_events.len());
        self.voices.clear();
    }

    pub fn set_tab_events(&mut self, mut ev: Vec<TabEvent>) {
        ev.sort_by_key(|e| e.at);
        self.tab_events = ev;
        let p = self.pos;
        self.seek(p);
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn pluck(&mut self, midi: i32, len: u64) {
        let f = 440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0);
        let n = (self.sr / f).max(2.0) as usize;
        let buf: Vec<f32> = (0..n).map(|_| self.rand() * 0.5).collect();
        let left = len.min((self.sr * 3.0) as u64) as usize;
        if self.voices.len() > 24 {
            self.voices.remove(0);
        }
        self.voices.push(Voice { buf, idx: 0, left, amp: 1.0 });
    }

    /// Render one output frame (mono).
    fn frame(&mut self) -> f32 {
        let mut out = 0.0;
        let live = {
            let mut inp = None;
            if let Ok(mut q) = self.input.try_lock() {
                inp = q.pop_front();
                if q.len() > 4096 {
                    let extra = q.len() - 1024;
                    q.drain(..extra);
                }
            }
            inp
        };
        if let Some(x) = live {
            let wet = self.chain.process(x);
            if self.monitor {
                out += wet * self.monitor_level;
            }
            if self.recording && self.playing {
                self.rec_buf.push(wet);
            }
        }
        if self.playing {
            let any_solo = self.tracks.iter().any(|t| t.solo);
            let p = self.pos as usize;
            for t in &self.tracks {
                if t.mute || (any_solo && !t.solo) {
                    continue;
                }
                if let Some(v) = t.samples.get(p) {
                    out += v * t.volume;
                }
            }
            // tab notes
            while self.next_tab < self.tab_events.len() && self.tab_events[self.next_tab].at <= self.pos {
                let (m, l) = (self.tab_events[self.next_tab].midi, self.tab_events[self.next_tab].len);
                self.pluck(m, l);
                self.next_tab += 1;
            }
            // metronome
            if self.metronome {
                let spb = (self.sr * 60.0 / self.tempo) as u64;
                let ph = self.pos % spb.max(1);
                if ph < (self.sr * 0.03) as u64 {
                    let beat = self.pos / spb.max(1);
                    let f = if beat % self.beats_per_bar as u64 == 0 { 1600.0 } else { 1000.0 };
                    let t = ph as f32 / self.sr;
                    out += (t * f * std::f32::consts::TAU).sin() * (1.0 - t / 0.03) * 0.35;
                }
            }
            self.pos += 1;
        }
        let mut syn = 0.0;
        for v in self.voices.iter_mut() {
            let n = v.buf.len();
            let a = v.buf[v.idx];
            let b = v.buf[(v.idx + 1) % n];
            v.buf[v.idx] = (a + b) * 0.497;
            v.idx = (v.idx + 1) % n;
            syn += a * v.amp;
            if v.left > 0 {
                v.left -= 1;
            } else {
                v.amp *= 0.998;
            }
        }
        self.voices.retain(|v| v.amp > 0.001);
        out += syn * self.tab_volume;
        out *= self.master;
        self.level = self.level * 0.9995 + out.abs() * 0.0005;
        out.clamp(-1.0, 1.0)
    }
}

pub struct Audio {
    pub engine: Arc<Mutex<Engine>>,
    _out: Option<cpal::Stream>,
    _in: Option<cpal::Stream>,
    pub status: String,
    pub input_status: String,
}

impl Audio {
    /// Start the output. With `enabled == false` (screenshot mode) no device is opened.
    pub fn start(tone: &ToneSettings, enabled: bool) -> Self {
        let mut sr = 48000.0;
        let host = cpal::default_host();
        let dev = if enabled { host.default_output_device() } else { None };
        if let Some(d) = dev.as_ref() {
            if let Ok(c) = d.default_output_config() {
                sr = c.sample_rate() as f32;
            }
        }
        let engine = Arc::new(Mutex::new(Engine::new(sr, tone)));
        let mut status = "Audio off".to_string();
        let mut out_stream = None;
        if let Some(d) = dev {
            match d.default_output_config() {
                Ok(cfg) => {
                    let channels = cfg.channels() as usize;
                    let config: cpal::StreamConfig = cfg.clone().into();
                    let eng = engine.clone();
                    let res = d.build_output_stream::<f32, _, _>(
                        config,
                        move |data: &mut [f32], _| {
                            if let Ok(mut e) = eng.lock() {
                                for fr in data.chunks_mut(channels) {
                                    let v = e.frame();
                                    for s in fr.iter_mut() {
                                        *s = v;
                                    }
                                }
                            }
                        },
                        |err| eprintln!("audio error: {err}"),
                        None,
                    );
                    match res {
                        Ok(s) => {
                            let _ = s.play();
                            status = format!("Output {:.0} Hz", sr);
                            out_stream = Some(s);
                        }
                        Err(e) => status = format!("No audio output: {e}"),
                    }
                }
                Err(e) => status = format!("No audio output: {e}"),
            }
        }
        Self { engine, _out: out_stream, _in: None, status, input_status: "Guitar input off".into() }
    }

    pub fn input_on(&self) -> bool {
        self._in.is_some()
    }

    /// Open the default input (audio interface / mic) and feed the rig and tuner.
    pub fn start_input(&mut self) {
        if self._in.is_some() {
            return;
        }
        let host = cpal::default_host();
        let Some(d) = host.default_input_device() else {
            self.input_status = "No input device found".into();
            return;
        };
        let cfg = match d.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                self.input_status = format!("Input error: {e}");
                return;
            }
        };
        let ch = cfg.channels() as usize;
        let (q, tq) = {
            let e = self.engine.lock().unwrap();
            (e.input.clone(), e.tuner_buf.clone())
        };
        let config: cpal::StreamConfig = cfg.clone().into();
        let res = d.build_input_stream::<f32, _, _>(
            config,
            move |data: &[f32], _| {
                if let Ok(mut qq) = q.lock() {
                    for fr in data.chunks(ch) {
                        qq.push_back(fr[0]);
                    }
                }
                if let Ok(mut t) = tq.lock() {
                    for fr in data.chunks(ch) {
                        t.push_back(fr[0]);
                    }
                    while t.len() > 4096 {
                        t.pop_front();
                    }
                }
            },
            |err| eprintln!("input error: {err}"),
            None,
        );
        match res {
            Ok(s) => {
                let _ = s.play();
                self.input_status = format!("Guitar input on ({} Hz)", cfg.sample_rate());
                self._in = Some(s);
            }
            Err(e) => self.input_status = format!("Input error: {e}"),
        }
    }

    pub fn stop_input(&mut self) {
        self._in = None;
        self.input_status = "Guitar input off".into();
    }
}

/// Read a WAV file to mono samples at `sr`.
pub fn load_wav(path: &str, sr: f32) -> Result<Vec<f32>, String> {
    let mut r = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = r.spec();
    let ch = spec.channels as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().filter_map(|s| s.ok()).map(|v| v as f32 / scale).collect()
        }
    };
    let mono: Vec<f32> = raw.chunks(ch.max(1)).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
    Ok(resample(&mono, spec.sample_rate as f32, sr))
}

pub fn resample(x: &[f32], from: f32, to: f32) -> Vec<f32> {
    if (from - to).abs() < 1.0 || x.is_empty() {
        return x.to_vec();
    }
    let ratio = from / to;
    let n = (x.len() as f32 / ratio) as usize;
    (0..n)
        .map(|i| {
            let p = i as f32 * ratio;
            let j = p as usize;
            let f = p - j as f32;
            let a = x[j.min(x.len() - 1)];
            let b = x[(j + 1).min(x.len() - 1)];
            a + (b - a) * f
        })
        .collect()
}

pub fn save_wav(path: &str, samples: &[f32], sr: f32) -> Result<(), String> {
    let spec = hound::WavSpec { channels: 1, sample_rate: sr as u32, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}
