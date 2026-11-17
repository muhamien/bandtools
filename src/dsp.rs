//! Guitar tone engine: amp models, cabinets, microphones and pedals.
//! Knob ranges follow MECHANICS.md (amp knobs 0-10, output gain in dB,
//! up to 14 pedal slots). The DSP itself is our own design.

use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

pub const MAX_PEDALS: usize = 14;

// ---------------------------------------------------------------- models

#[derive(Clone, Copy, PartialEq)]
pub enum Shape {
    Soft,
    Asym,
    Hard,
}

pub struct AmpModel {
    pub name: &'static str,
    pub style: &'static str,
    pub max_gain_db: f32,
    pub stages: u8,
    pub shape: Shape,
    pub mid_hz: f32,
    pub bright: f32,
    pub default_cab: usize,
}

pub const AMPS: &[AmpModel] = &[
    AmpModel { name: "Tweed Combo", style: "1950s American combo, warm and touchy", max_gain_db: 34.0, stages: 1, shape: Shape::Asym, mid_hz: 700.0, bright: 0.0, default_cab: 4 },
    AmpModel { name: "Blackface Clean", style: "1960s American clean with sparkle", max_gain_db: 26.0, stages: 1, shape: Shape::Soft, mid_hz: 500.0, bright: 2.0, default_cab: 1 },
    AmpModel { name: "Top Boost Combo", style: "British class-A chime", max_gain_db: 32.0, stages: 1, shape: Shape::Asym, mid_hz: 900.0, bright: 3.0, default_cab: 2 },
    AmpModel { name: "Plexi Stack", style: "Late-60s British rock stack", max_gain_db: 38.0, stages: 2, shape: Shape::Asym, mid_hz: 750.0, bright: 1.5, default_cab: 5 },
    AmpModel { name: "British Lead", style: "80s British hard-rock lead", max_gain_db: 44.0, stages: 2, shape: Shape::Hard, mid_hz: 800.0, bright: 1.0, default_cab: 5 },
    AmpModel { name: "Brown Hot-Rod", style: "Modded British, 'brown' saturated crunch", max_gain_db: 46.0, stages: 2, shape: Shape::Asym, mid_hz: 650.0, bright: 0.5, default_cab: 5 },
    AmpModel { name: "Boutique Smooth", style: "Hand-wired US boutique, vocal leads", max_gain_db: 40.0, stages: 2, shape: Shape::Soft, mid_hz: 600.0, bright: 0.0, default_cab: 3 },
    AmpModel { name: "Modern High-Gain", style: "Tight modern metal, scooped", max_gain_db: 54.0, stages: 3, shape: Shape::Hard, mid_hz: 700.0, bright: 1.0, default_cab: 6 },
    AmpModel { name: "Metal Lead 51", style: "90s US high-gain lead channel", max_gain_db: 56.0, stages: 3, shape: Shape::Asym, mid_hz: 850.0, bright: 1.5, default_cab: 6 },
    AmpModel { name: "Solid-State Jazz", style: "Clean transistor amp, huge headroom", max_gain_db: 18.0, stages: 1, shape: Shape::Hard, mid_hz: 500.0, bright: 1.0, default_cab: 0 },
];

pub struct Cabinet {
    pub name: &'static str,
    pub low_hz: f32,
    pub reso_hz: f32,
    pub high_hz: f32,
    pub notch_hz: f32,
}

pub const CABS: &[Cabinet] = &[
    Cabinet { name: "1x12 Open Back", low_hz: 90.0, reso_hz: 110.0, high_hz: 5600.0, notch_hz: 1200.0 },
    Cabinet { name: "2x12 Open Back", low_hz: 80.0, reso_hz: 100.0, high_hz: 5400.0, notch_hz: 1100.0 },
    Cabinet { name: "2x12 Blue Alnico", low_hz: 85.0, reso_hz: 105.0, high_hz: 6200.0, notch_hz: 1500.0 },
    Cabinet { name: "1x12 Boutique Closed", low_hz: 75.0, reso_hz: 95.0, high_hz: 5000.0, notch_hz: 1000.0 },
    Cabinet { name: "4x10 Tweed", low_hz: 85.0, reso_hz: 120.0, high_hz: 5200.0, notch_hz: 1300.0 },
    Cabinet { name: "4x12 British Vintage", low_hz: 70.0, reso_hz: 90.0, high_hz: 4800.0, notch_hz: 900.0 },
    Cabinet { name: "4x12 Modern Closed", low_hz: 65.0, reso_hz: 80.0, high_hz: 5600.0, notch_hz: 800.0 },
    Cabinet { name: "1x15 Jazz", low_hz: 60.0, reso_hz: 75.0, high_hz: 4200.0, notch_hz: 1000.0 },
];

pub const MICS: &[&str] = &["Dynamic (SM-style)", "Condenser", "Ribbon"];

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum PedalKind {
    NoiseGate,
    Compressor,
    TrebleBoost,
    GreenDrive,
    YellowDist,
    GritBox,
    TriangleFuzz,
    Octaver,
    AutoWah,
    Chorus,
    Phaser,
    Flanger,
    Tremolo,
    AnalogDelay,
    TapeEcho,
    SpringVerb,
}

pub struct PedalInfo {
    pub kind: PedalKind,
    pub name: &'static str,
    pub like: &'static str,
    pub knobs: [&'static str; 3],
    pub color: [u8; 3],
    pub category: &'static str,
}

pub const PEDALS: &[PedalInfo] = &[
    PedalInfo { kind: PedalKind::NoiseGate, name: "Quiet Gate", like: "noise suppressor", knobs: ["Threshold", "Release", "Level"], color: [60, 60, 66], category: "Dynamics" },
    PedalInfo { kind: PedalKind::Compressor, name: "Squeeze Comp", like: "classic orange-box compressor", knobs: ["Sustain", "Attack", "Level"], color: [222, 110, 40], category: "Dynamics" },
    PedalInfo { kind: PedalKind::TrebleBoost, name: "Treble Booster", like: "60s germanium treble boost", knobs: ["Boost", "Tone", "Level"], color: [180, 180, 190], category: "Drive" },
    PedalInfo { kind: PedalKind::GreenDrive, name: "Green Drive", like: "mid-hump green overdrive", knobs: ["Drive", "Tone", "Level"], color: [60, 160, 90], category: "Drive" },
    PedalInfo { kind: PedalKind::YellowDist, name: "Yellow Distortion", like: "yellow-box hard clipper", knobs: ["Dist", "Tone", "Level"], color: [235, 200, 40], category: "Drive" },
    PedalInfo { kind: PedalKind::GritBox, name: "Grit Box", like: "black op-amp distortion", knobs: ["Distortion", "Filter", "Volume"], color: [30, 30, 30], category: "Drive" },
    PedalInfo { kind: PedalKind::TriangleFuzz, name: "Triangle Fuzz", like: "big silicon sustain fuzz", knobs: ["Sustain", "Tone", "Volume"], color: [200, 200, 205], category: "Drive" },
    PedalInfo { kind: PedalKind::Octaver, name: "Sub Octave", like: "analog octave down", knobs: ["Sub", "Dry", "Level"], color: [70, 90, 160], category: "Pitch" },
    PedalInfo { kind: PedalKind::AutoWah, name: "Auto Wah", like: "envelope / cry wah", knobs: ["Sens", "Range", "Mix"], color: [40, 40, 40], category: "Filter" },
    PedalInfo { kind: PedalKind::Chorus, name: "Aqua Chorus", like: "blue-box chorus", knobs: ["Rate", "Depth", "Mix"], color: [70, 150, 220], category: "Modulation" },
    PedalInfo { kind: PedalKind::Phaser, name: "Phase 90°", like: "orange 4-stage phaser", knobs: ["Speed", "Depth", "Feedback"], color: [240, 120, 30], category: "Modulation" },
    PedalInfo { kind: PedalKind::Flanger, name: "Jet Flanger", like: "80s jet flanger", knobs: ["Rate", "Depth", "Regen"], color: [150, 80, 170], category: "Modulation" },
    PedalInfo { kind: PedalKind::Tremolo, name: "Tremolo", like: "amp-style tremolo", knobs: ["Speed", "Depth", "Shape"], color: [200, 60, 70], category: "Modulation" },
    PedalInfo { kind: PedalKind::AnalogDelay, name: "Bucket Delay", like: "warm analog delay", knobs: ["Time", "Repeats", "Mix"], color: [40, 120, 110], category: "Time" },
    PedalInfo { kind: PedalKind::TapeEcho, name: "Tape Echo", like: "vintage tape echo with wow", knobs: ["Time", "Repeats", "Mix"], color: [110, 85, 60], category: "Time" },
    PedalInfo { kind: PedalKind::SpringVerb, name: "Spring Tank", like: "surf spring reverb", knobs: ["Dwell", "Tone", "Mix"], color: [190, 160, 110], category: "Time" },
];

pub fn pedal_info(k: PedalKind) -> &'static PedalInfo {
    PEDALS.iter().find(|p| p.kind == k).unwrap()
}

// ---------------------------------------------------------------- settings

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct AmpSettings {
    pub model: usize,
    pub gain: f32,
    pub bass: f32,
    pub mids: f32,
    pub treble: f32,
    pub presence: f32,
    pub master: f32,
    pub output_db: f32,
    pub cab: usize,
    pub mic: usize,
    pub mic_edge: f32,
    pub reverb_on: bool,
    pub reverb_level: f32,
}

impl Default for AmpSettings {
    /// Matches the original's default amp preset knob values.
    fn default() -> Self {
        Self { model: 3, gain: 4.5, bass: 6.7, mids: 3.1, treble: 7.4, presence: 8.3, master: 9.4, output_db: -7.0, cab: 5, mic: 0, mic_edge: 0.25, reverb_on: true, reverb_level: 2.2 }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct PedalSlot {
    pub kind: PedalKind,
    pub on: bool,
    pub knobs: [f32; 3],
}

impl PedalSlot {
    pub fn new(kind: PedalKind) -> Self {
        Self { kind, on: true, knobs: [0.5, 0.5, 0.5] }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct ToneSettings {
    pub amp: AmpSettings,
    pub pedals: Vec<PedalSlot>,
    pub amp_on: bool,
}

impl Default for ToneSettings {
    fn default() -> Self {
        Self {
            amp: AmpSettings::default(),
            pedals: vec![
                PedalSlot { kind: PedalKind::GreenDrive, on: false, knobs: [0.4, 0.5, 0.6] },
                PedalSlot { kind: PedalKind::Chorus, on: false, knobs: [0.3, 0.5, 0.4] },
                PedalSlot { kind: PedalKind::AnalogDelay, on: true, knobs: [0.35, 0.3, 0.25] },
            ],
            amp_on: true,
        }
    }
}

// ---------------------------------------------------------------- filters

#[derive(Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn set(&mut self, b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) {
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }
    pub fn lowpass(&mut self, sr: f32, f: f32, q: f32) {
        let w = 2.0 * PI * f.min(sr * 0.45) / sr;
        let (s, c) = w.sin_cos();
        let a = s / (2.0 * q);
        self.set((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0, 1.0 + a, -2.0 * c, 1.0 - a);
    }
    pub fn highpass(&mut self, sr: f32, f: f32, q: f32) {
        let w = 2.0 * PI * f.min(sr * 0.45) / sr;
        let (s, c) = w.sin_cos();
        let a = s / (2.0 * q);
        self.set((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0, 1.0 + a, -2.0 * c, 1.0 - a);
    }
    pub fn bandpass(&mut self, sr: f32, f: f32, q: f32) {
        let w = 2.0 * PI * f.clamp(20.0, sr * 0.45) / sr;
        let (s, c) = w.sin_cos();
        let a = s / (2.0 * q);
        self.set(a, 0.0, -a, 1.0 + a, -2.0 * c, 1.0 - a);
    }
    pub fn peak(&mut self, sr: f32, f: f32, q: f32, db: f32) {
        let w = 2.0 * PI * f.min(sr * 0.45) / sr;
        let (s, c) = w.sin_cos();
        let a = s / (2.0 * q);
        let g = 10f32.powf(db / 40.0);
        self.set(1.0 + a * g, -2.0 * c, 1.0 - a * g, 1.0 + a / g, -2.0 * c, 1.0 - a / g);
    }
    pub fn shelf(&mut self, sr: f32, f: f32, db: f32, high: bool) {
        let w = 2.0 * PI * f.min(sr * 0.45) / sr;
        let (s, c) = w.sin_cos();
        let g = 10f32.powf(db / 40.0);
        let a = s / 2.0 * 2f32.sqrt();
        let sq = 2.0 * g.sqrt() * a;
        if high {
            self.set(
                g * ((g + 1.0) + (g - 1.0) * c + sq),
                -2.0 * g * ((g - 1.0) + (g + 1.0) * c),
                g * ((g + 1.0) + (g - 1.0) * c - sq),
                (g + 1.0) - (g - 1.0) * c + sq,
                2.0 * ((g - 1.0) - (g + 1.0) * c),
                (g + 1.0) - (g - 1.0) * c - sq,
            );
        } else {
            self.set(
                g * ((g + 1.0) - (g - 1.0) * c + sq),
                2.0 * g * ((g - 1.0) - (g + 1.0) * c),
                g * ((g + 1.0) - (g - 1.0) * c - sq),
                (g + 1.0) + (g - 1.0) * c + sq,
                -2.0 * ((g - 1.0) + (g + 1.0) * c),
                (g + 1.0) + (g - 1.0) * c - sq,
            );
        }
    }
    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        if !y.is_finite() {
            self.z1 = 0.0;
            self.z2 = 0.0;
            return 0.0;
        }
        y
    }
}

fn db(d: f32) -> f32 {
    10f32.powf(d / 20.0)
}

fn shape(x: f32, s: Shape) -> f32 {
    match s {
        Shape::Soft => x.tanh(),
        Shape::Asym => {
            if x >= 0.0 {
                x.tanh()
            } else {
                (x * 0.8).tanh() * 1.1
            }
        }
        Shape::Hard => (x * 1.5).clamp(-1.2, 1.2).tanh() * 1.05,
    }
}

// ---------------------------------------------------------------- delay line

#[derive(Clone)]
struct DelayLine {
    buf: Vec<f32>,
    w: usize,
}

impl DelayLine {
    fn new(n: usize) -> Self {
        Self { buf: vec![0.0; n.max(4)], w: 0 }
    }
    fn push(&mut self, x: f32) {
        self.buf[self.w] = x;
        self.w = (self.w + 1) % self.buf.len();
    }
    /// Read `d` samples back (fractional).
    fn read(&self, d: f32) -> f32 {
        let n = self.buf.len();
        let d = d.clamp(1.0, (n - 2) as f32);
        let i = d.floor() as usize;
        let f = d - i as f32;
        let a = self.buf[(self.w + n - i) % n];
        let b = self.buf[(self.w + n - i - 1) % n];
        a + (b - a) * f
    }
}

// ---------------------------------------------------------------- reverb

struct Reverb {
    combs: Vec<(DelayLine, usize, f32)>,
    aps: Vec<(DelayLine, usize)>,
}

impl Reverb {
    fn new(sr: f32) -> Self {
        let k = sr / 44100.0;
        let combs = [1116, 1188, 1277, 1356]
            .iter()
            .map(|&n| {
                let len = (n as f32 * k) as usize;
                (DelayLine::new(len + 2), len, 0.0)
            })
            .collect();
        let aps = [556, 441]
            .iter()
            .map(|&n| {
                let len = (n as f32 * k) as usize;
                (DelayLine::new(len + 2), len)
            })
            .collect();
        Self { combs, aps }
    }
    fn run(&mut self, x: f32, size: f32, damp: f32) -> f32 {
        let mut out = 0.0;
        for (dl, len, lp) in self.combs.iter_mut() {
            let y = dl.read(*len as f32);
            *lp = y * (1.0 - damp) + *lp * damp;
            dl.push(x * 0.3 + *lp * size);
            out += y;
        }
        for (dl, len) in self.aps.iter_mut() {
            let b = dl.read(*len as f32);
            dl.push(out + b * 0.5);
            out = b - out * 0.5;
        }
        out * 0.25
    }
}

// ---------------------------------------------------------------- pedal state

struct PedalState {
    f1: Biquad,
    f2: Biquad,
    f3: Biquad,
    phase: f32,
    env: f32,
    gate: f32,
    dl: DelayLine,
    ap: [f32; 4],
    sign: f32,
    last: f32,
    fb: f32,
    rev: Option<Reverb>,
}

impl PedalState {
    fn new(sr: f32) -> Self {
        Self {
            f1: Biquad::default(),
            f2: Biquad::default(),
            f3: Biquad::default(),
            phase: 0.0,
            env: 0.0,
            gate: 1.0,
            dl: DelayLine::new((sr * 1.6) as usize),
            ap: [0.0; 4],
            sign: 1.0,
            last: 0.0,
            fb: 0.0,
            rev: None,
        }
    }
}

// ---------------------------------------------------------------- chain

pub struct ToneChain {
    sr: f32,
    settings: ToneSettings,
    pre_hp: Biquad,
    inter: Biquad,
    bass: Biquad,
    mid: Biquad,
    treble: Biquad,
    presence: Biquad,
    cab_hp: Biquad,
    cab_reso: Biquad,
    cab_notch: Biquad,
    cab_lp: Biquad,
    mic: Biquad,
    reverb: Reverb,
    pedals: Vec<PedalState>,
    pub meter: f32,
}

impl ToneChain {
    pub fn new(sr: f32, settings: &ToneSettings) -> Self {
        let mut c = Self {
            sr,
            settings: settings.clone(),
            pre_hp: Biquad::default(),
            inter: Biquad::default(),
            bass: Biquad::default(),
            mid: Biquad::default(),
            treble: Biquad::default(),
            presence: Biquad::default(),
            cab_hp: Biquad::default(),
            cab_reso: Biquad::default(),
            cab_notch: Biquad::default(),
            cab_lp: Biquad::default(),
            mic: Biquad::default(),
            reverb: Reverb::new(sr),
            pedals: (0..MAX_PEDALS).map(|_| PedalState::new(sr)).collect(),
            meter: 0.0,
        };
        c.apply(settings);
        c
    }

    pub fn apply(&mut self, s: &ToneSettings) {
        let sr = self.sr;
        let a = &s.amp;
        let m = &AMPS[a.model.min(AMPS.len() - 1)];
        let cab = &CABS[a.cab.min(CABS.len() - 1)];
        self.pre_hp.highpass(sr, 70.0, 0.7);
        self.inter.lowpass(sr, 6500.0 - m.stages as f32 * 900.0, 0.7);
        self.bass.shelf(sr, 120.0, (a.bass - 5.0) * 1.8, false);
        self.mid.peak(sr, m.mid_hz, 0.8, (a.mids - 5.0) * 1.8);
        self.treble.shelf(sr, 2800.0, (a.treble - 5.0) * 1.6 + m.bright, true);
        self.presence.shelf(sr, 5000.0, (a.presence - 5.0) * 1.2, true);
        self.cab_hp.highpass(sr, cab.low_hz, 0.9);
        self.cab_reso.peak(sr, cab.reso_hz, 1.5, 4.0);
        self.cab_notch.peak(sr, cab.notch_hz, 1.2, -3.0);
        let edge = a.mic_edge.clamp(0.0, 1.0);
        let lp = cab.high_hz * (1.0 - 0.35 * edge);
        self.cab_lp.lowpass(sr, lp, 0.9);
        match a.mic {
            0 => self.mic.peak(sr, 4500.0, 1.0, 3.0),
            1 => self.mic.shelf(sr, 6000.0, 2.0, true),
            _ => self.mic.shelf(sr, 3000.0, -4.0, true),
        }
        for (i, p) in s.pedals.iter().enumerate().take(MAX_PEDALS) {
            let st = &mut self.pedals[i];
            let k = p.knobs;
            match p.kind {
                PedalKind::TrebleBoost => st.f1.highpass(sr, 400.0 + 1200.0 * k[1], 0.7),
                PedalKind::GreenDrive => {
                    st.f1.highpass(sr, 720.0, 0.6);
                    st.f2.lowpass(sr, 1200.0 + 4000.0 * k[1], 0.7);
                }
                PedalKind::YellowDist | PedalKind::GritBox => {
                    st.f1.highpass(sr, 90.0, 0.7);
                    let f = if p.kind == PedalKind::GritBox { 5500.0 - 4500.0 * k[1] } else { 1500.0 + 4000.0 * k[1] };
                    st.f2.lowpass(sr, f, 0.7);
                }
                PedalKind::TriangleFuzz => {
                    st.f1.lowpass(sr, 700.0, 0.7);
                    st.f2.highpass(sr, 900.0, 0.7);
                    st.f3.peak(sr, 1000.0, 0.7, -9.0 * (1.0 - (k[1] - 0.5).abs() * 2.0));
                }
                PedalKind::Octaver => st.f1.lowpass(sr, 300.0, 0.7),
                PedalKind::SpringVerb => {
                    if st.rev.is_none() {
                        st.rev = Some(Reverb::new(sr));
                    }
                    st.f1.lowpass(sr, 2000.0 + 5000.0 * k[1], 0.7);
                    st.f2.highpass(sr, 300.0, 0.7);
                }
                PedalKind::AnalogDelay => st.f1.lowpass(sr, 2800.0, 0.7),
                PedalKind::TapeEcho => st.f1.lowpass(sr, 4000.0, 0.7),
                _ => {}
            }
        }
        self.settings = s.clone();
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let sr = self.sr;
        let mut y = x;
        let n = self.settings.pedals.len().min(MAX_PEDALS);
        for i in 0..n {
            let p = self.settings.pedals[i].clone();
            if p.on {
                y = run_pedal(&p, &mut self.pedals[i], y, sr);
            }
        }
        if self.settings.amp_on {
            y = self.run_amp(y);
        }
        let a = y.abs();
        self.meter = self.meter * 0.999 + a * 0.001;
        y.clamp(-1.0, 1.0)
    }

    fn run_amp(&mut self, x: f32) -> f32 {
        let a = &self.settings.amp;
        let m = &AMPS[a.model.min(AMPS.len() - 1)];
        let mut y = self.pre_hp.run(x);
        let drive = db(a.gain / 10.0 * m.max_gain_db);
        y *= drive;
        for st in 0..m.stages {
            y = shape(y, m.shape);
            if st + 1 < m.stages {
                y = self.inter.run(y) * 2.5;
            }
        }
        y /= drive.powf(0.55).max(1.0);
        y = self.bass.run(y);
        y = self.mid.run(y);
        y = self.treble.run(y);
        y = self.presence.run(y);
        let master = 0.4 + a.master / 10.0 * 1.6;
        y = (y * master).tanh();
        y = self.cab_hp.run(y);
        y = self.cab_reso.run(y);
        y = self.cab_notch.run(y);
        y = self.cab_lp.run(y);
        y = self.mic.run(y);
        if a.reverb_on {
            let wet = self.reverb.run(y, 0.82, 0.3);
            y += wet * a.reverb_level / 10.0 * 1.4;
        }
        y * db(a.output_db + 6.0)
    }
}

fn run_pedal(p: &PedalSlot, st: &mut PedalState, x: f32, sr: f32) -> f32 {
    let k = p.knobs;
    match p.kind {
        PedalKind::NoiseGate => {
            st.env = st.env.max(x.abs()) * (1.0 - 1.0 / (sr * 0.02));
            let thr = db(-80.0 + 60.0 * k[0]);
            let target = if st.env > thr { 1.0 } else { 0.0 };
            let rel = 1.0 / (sr * (0.02 + 0.3 * k[1]));
            st.gate += (target - st.gate) * if target > st.gate { 0.01 } else { rel };
            x * st.gate * (0.5 + k[2])
        }
        PedalKind::Compressor => {
            let a = x.abs();
            let att = 1.0 / (sr * (0.001 + 0.03 * k[1]));
            let rel = 1.0 / (sr * 0.15);
            st.env += (a - st.env) * if a > st.env { att } else { rel };
            let thr = db(-10.0 - 30.0 * k[0]);
            let gain = if st.env > thr { (thr / st.env).powf(0.75) } else { 1.0 };
            x * gain * db(18.0 * k[0]) * (0.3 + k[2] * 1.4)
        }
        PedalKind::TrebleBoost => {
            let h = st.f1.run(x);
            (x * 0.5 + h * (1.0 + 8.0 * k[0])).tanh() * (0.3 + k[2] * 1.2)
        }
        PedalKind::GreenDrive => {
            let h = st.f1.run(x);
            let d = (h * db(6.0 + 34.0 * k[0])).tanh();
            st.f2.run(x * 0.6 + d * 0.5) * (0.3 + k[2] * 1.4)
        }
        PedalKind::YellowDist | PedalKind::GritBox => {
            let h = st.f1.run(x) * db(10.0 + 40.0 * k[0]);
            let d = h.clamp(-0.7, 0.7) / 0.7;
            st.f2.run(d) * (0.2 + k[2] * 1.0)
        }
        PedalKind::TriangleFuzz => {
            let mut d = (x * db(20.0 + 35.0 * k[0])).tanh();
            d = (d * 3.0).tanh();
            let lo = st.f1.run(d);
            let hi = st.f2.run(d);
            let t = k[1];
            st.f3.run(lo * (1.0 - t) + hi * t) * (0.2 + k[2] * 1.2)
        }
        PedalKind::Octaver => {
            let l = st.f1.run(x);
            if st.last <= 0.0 && l > 0.0 {
                st.sign = -st.sign;
            }
            st.last = l;
            st.env = st.env * 0.999 + l.abs() * 0.001;
            let sub = st.sign * st.env * 4.0;
            (x * k[1] * 1.5 + sub * k[0] * 2.0) * (0.4 + k[2] * 1.2)
        }
        PedalKind::AutoWah => {
            st.env = st.env * 0.995 + x.abs() * 0.005;
            let f = 350.0 + (st.env * (20.0 + 200.0 * k[0])).min(1.0) * (600.0 + 2400.0 * k[1]);
            st.f1.bandpass(sr, f, 4.0);
            let w = st.f1.run(x) * 3.0;
            x * (1.0 - k[2]) + w * k[2]
        }
        PedalKind::Chorus | PedalKind::Flanger => {
            let fl = p.kind == PedalKind::Flanger;
            let rate = if fl { 0.05 + 1.5 * k[0] } else { 0.2 + 3.0 * k[0] };
            st.phase = (st.phase + rate / sr) % 1.0;
            let lfo = (st.phase * 2.0 * PI).sin() * 0.5 + 0.5;
            let base = if fl { 0.001 } else { 0.007 };
            let range = if fl { 0.004 } else { 0.006 };
            let d = (base + range * k[1] * lfo) * sr;
            let wet = st.dl.read(d);
            let fb = if fl { k[2] * 0.85 } else { 0.0 };
            st.dl.push(x + wet * fb);
            let mix = if fl { 0.5 } else { k[2] * 0.6 };
            x * (1.0 - mix * 0.5) + wet * mix
        }
        PedalKind::Phaser => {
            st.phase = (st.phase + (0.1 + 4.0 * k[0]) / sr) % 1.0;
            let lfo = (st.phase * 2.0 * PI).sin() * 0.5 + 0.5;
            let f = 200.0 + 1600.0 * lfo * (0.3 + 0.7 * k[1]);
            let t = (PI * f / sr).tan();
            let c = (t - 1.0) / (t + 1.0);
            let mut y = x + st.fb * k[2] * 0.7;
            for z in st.ap.iter_mut() {
                let o = c * y + *z;
                *z = y - c * o;
                y = o;
            }
            st.fb = y;
            (x + y) * 0.6
        }
        PedalKind::Tremolo => {
            st.phase = (st.phase + (1.0 + 11.0 * k[0]) / sr) % 1.0;
            let sine = (st.phase * 2.0 * PI).sin() * 0.5 + 0.5;
            let square = if st.phase < 0.5 { 1.0 } else { 0.0 };
            let l = sine * (1.0 - k[2]) + square * k[2];
            x * (1.0 - k[1] * l)
        }
        PedalKind::AnalogDelay | PedalKind::TapeEcho => {
            let tape = p.kind == PedalKind::TapeEcho;
            let mut d = (0.04 + 0.96 * k[0]) * sr * if tape { 0.9 } else { 0.6 };
            if tape {
                st.phase = (st.phase + 0.7 / sr) % 1.0;
                d *= 1.0 + 0.003 * (st.phase * 2.0 * PI).sin();
            }
            let wet = st.dl.read(d);
            let fbv = st.f1.run(wet) * (k[1] * 0.9);
            st.dl.push(if tape { (x + fbv).tanh() } else { x + fbv });
            x + wet * k[2] * 1.2
        }
        PedalKind::SpringVerb => {
            let inp = st.f2.run(x);
            let wet = match st.rev.as_mut() {
                Some(r) => r.run(inp, 0.7 + 0.25 * k[0], 0.2),
                None => 0.0,
            };
            let wet = st.f1.run(wet);
            x + wet * k[2] * 2.5
        }
    }
}

/// Pitch detection (YIN) used by the tuner and the tab recognizer.
pub fn detect_pitch(buf: &[f32], sr: f32) -> Option<(f32, f32)> {
    let n = buf.len() / 2;
    if n < 64 {
        return None;
    }
    let rms = (buf.iter().map(|v| v * v).sum::<f32>() / buf.len() as f32).sqrt();
    if rms < 0.004 {
        return None;
    }
    let min_lag = (sr / 1400.0) as usize;
    let max_lag = ((sr / 60.0) as usize).min(n - 1);
    let mut d = vec![0.0f32; max_lag + 1];
    for tau in 1..=max_lag {
        let mut s = 0.0;
        for i in 0..n {
            let v = buf[i] - buf[i + tau];
            s += v * v;
        }
        d[tau] = s;
    }
    let mut cm = vec![1.0f32; max_lag + 1];
    let mut run = 0.0;
    for tau in 1..=max_lag {
        run += d[tau];
        cm[tau] = if run > 0.0 { d[tau] * tau as f32 / run } else { 1.0 };
    }
    let mut tau = min_lag.max(2);
    while tau < max_lag {
        if cm[tau] < 0.15 {
            while tau + 1 < max_lag && cm[tau + 1] < cm[tau] {
                tau += 1;
            }
            let (a, b, c) = (cm[tau - 1], cm[tau], cm[tau + 1]);
            let den = a + c - 2.0 * b;
            let shift = if den.abs() > 1e-9 { 0.5 * (a - c) / den } else { 0.0 };
            return Some((sr / (tau as f32 + shift), rms));
        }
        tau += 1;
    }
    None
}

pub const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// (midi note, cents off) for a frequency, with A4 = `a4` Hz.
pub fn freq_to_note(f: f32, a4: f32) -> (i32, f32) {
    let m = 69.0 + 12.0 * (f / a4).log2();
    let r = m.round();
    (r as i32, (m - r) * 100.0)
}

/// Flat spellings; the original keeps a sharp table and a flat table and picks one
/// by a flag (-[DfDocument _updateTunerStrings] @0x100e04640).
pub const NOTE_NAMES_FLAT: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];

/// In-tune window: the original treats |deviation| <= 3 cents as correct
/// (-[DfChordTrainerWindowController _tunerTimerCallback:] @0x100d8914c, `dev + 3 > 6` test).
pub const IN_TUNE_CENTS: f32 = 3.0;
/// Needle travel: clamped to +/-50 cents (same function, constants 50.0 / -50.0).
pub const NEEDLE_CENTS: f32 = 50.0;

/// Name for a MIDI note: `midi % 12` indexes a 12-entry table; a negative note gives
/// an empty name (-[DfDocument tunerStringForMidiNote:] @0x100e047b4).
pub fn note_name_in(midi: i32, flats: bool) -> String {
    if midi < 0 {
        return String::new();
    }
    let table = if flats { &NOTE_NAMES_FLAT } else { &NOTE_NAMES };
    format!("{}{}", table[(midi % 12) as usize], midi / 12 - 1)
}
