//! AI helpers: tone setup from words (local open-source LLM through Ollama,
//! with a built-in fallback), tone matching from a guitar stem, and
//! audio-to-tab recognition (Basic Pitch when installed, built-in pitch
//! tracker otherwise).

use crate::dsp::{self, AmpSettings, PedalKind, PedalSlot, ToneSettings, AMPS, CABS, PEDALS};
use crate::tabs::{self, HeardNote, TabNote};

/// Open-source models we recommend, shown in the AI page.
pub const MODEL_GUIDE: &[(&str, &str, &str)] = &[
    ("qwen2.5:7b", "Tone setup (default)", "Qwen 2.5 7B Instruct, Apache-2.0. Reliable JSON answers, runs on 8 GB RAM."),
    ("qwen3:8b", "Tone setup", "Qwen 3 8B, Apache-2.0. Stronger reasoning about gear and genres."),
    ("llama3.1:8b", "Tone setup", "Llama 3.1 8B Instruct (Llama community license). Good gear knowledge."),
    ("gemma3:4b", "Tone setup (light)", "Gemma 3 4B. Small and fast for older Macs."),
    ("mistral:7b", "Tone setup", "Mistral 7B Instruct, Apache-2.0."),
    ("basic-pitch", "Tab recognition", "Spotify Basic Pitch, Apache-2.0. Hears polyphonic notes and chords in audio."),
    ("htdemucs_6s", "Stem splitting", "Meta Demucs v4 six-stem model, MIT. Has a dedicated guitar stem."),
];

pub struct Preset {
    pub name: &'static str,
    pub group: &'static str,
    pub amp: usize,
    pub cab: usize,
    /// gain, bass, mids, treble, presence, master, output dB, reverb level
    pub k: [f32; 8],
    pub pedals: &'static [(PedalKind, [f32; 3])],
}

/// Knob values taken from the original's Clean / Crunch / Distorted amp presets
/// (see MECHANICS.md); names and pedal choices are ours.
pub const PRESETS: &[Preset] = &[
    Preset { name: "Glass Clean", group: "Clean", amp: 2, cab: 2, k: [4.4, 5.1, 7.0, 6.2, 6.4, 8.4, -5.0, 8.5], pedals: &[(PedalKind::Compressor, [0.4, 0.5, 0.5])] },
    Preset { name: "Warm Jazz", group: "Clean", amp: 9, cab: 7, k: [2.7, 7.2, 5.0, 0.0, 0.0, 10.0, -3.5, 1.5], pedals: &[] },
    Preset { name: "Country Snap", group: "Clean", amp: 1, cab: 1, k: [4.8, 5.4, 5.2, 7.0, 6.5, 7.2, -7.0, 6.5], pedals: &[(PedalKind::Compressor, [0.6, 0.3, 0.5]), (PedalKind::AnalogDelay, [0.15, 0.1, 0.25])] },
    Preset { name: "Studio Default", group: "Clean", amp: 3, cab: 5, k: [4.5, 6.7, 3.1, 7.4, 8.3, 9.4, -7.0, 2.2], pedals: &[] },
    Preset { name: "Vintage Stack Crunch", group: "Crunch", amp: 3, cab: 5, k: [7.1, 5.0, 5.0, 6.7, 4.3, 7.7, -4.0, 3.8], pedals: &[] },
    Preset { name: "Pushed British", group: "Crunch", amp: 2, cab: 2, k: [7.2, 7.1, 5.1, 10.0, 6.1, 8.3, -5.0, 4.1], pedals: &[(PedalKind::TrebleBoost, [0.5, 0.5, 0.5])] },
    Preset { name: "Rhythm Grind", group: "Crunch", amp: 5, cab: 5, k: [10.0, 5.6, 2.7, 5.6, 6.9, 7.9, -7.5, 5.0], pedals: &[(PedalKind::NoiseGate, [0.3, 0.3, 0.5])] },
    Preset { name: "Singing Lead", group: "Distorted", amp: 8, cab: 6, k: [10.0, 3.2, 7.7, 6.2, 4.7, 8.7, -4.0, 2.3], pedals: &[(PedalKind::GreenDrive, [0.2, 0.5, 0.7]), (PedalKind::AnalogDelay, [0.4, 0.35, 0.3])] },
    Preset { name: "Modern Rock Wall", group: "Distorted", amp: 7, cab: 6, k: [6.6, 5.3, 4.6, 4.1, 3.8, 5.7, -4.0, 2.3], pedals: &[(PedalKind::NoiseGate, [0.4, 0.3, 0.5]), (PedalKind::GreenDrive, [0.1, 0.6, 0.8])] },
    Preset { name: "High Octane", group: "Distorted", amp: 4, cab: 5, k: [8.0, 6.6, 5.5, 5.9, 6.2, 10.0, -8.0, 0.0], pedals: &[] },
];

pub fn apply_preset(p: &Preset) -> ToneSettings {
    let amp = AmpSettings {
        model: p.amp,
        gain: p.k[0],
        bass: p.k[1],
        mids: p.k[2],
        treble: p.k[3],
        presence: p.k[4],
        master: p.k[5],
        output_db: p.k[6],
        cab: p.cab,
        mic: 0,
        mic_edge: 0.25,
        reverb_on: p.k[7] > 0.0,
        reverb_level: p.k[7],
    };
    ToneSettings { amp, pedals: p.pedals.iter().map(|(k, v)| PedalSlot { kind: *k, on: true, knobs: *v }).collect(), amp_on: true }
}

/// The prompt sent to the local LLM.
pub fn tone_prompt(request: &str) -> String {
    let amps: Vec<String> = AMPS.iter().map(|a| format!("\"{}\" ({})", a.name, a.style)).collect();
    let cabs: Vec<&str> = CABS.iter().map(|c| c.name).collect();
    let peds: Vec<String> = PEDALS.iter().map(|p| format!("\"{}\" ({}; knobs {})", p.name, p.like, p.knobs.join("/"))).collect();
    format!(
        "You are a guitar tone engineer. Build a rig for this request: \"{request}\".\n\
Choose ONLY from these amps: {}.\nCabinets: {}.\nMics: Dynamic, Condenser, Ribbon.\nPedals: {}.\n\
Answer with JSON only: {{\"amp\":\"...\",\"cabinet\":\"...\",\"mic\":\"...\",\"gain\":0-10,\"bass\":0-10,\"mids\":0-10,\"treble\":0-10,\"presence\":0-10,\"master\":0-10,\"reverb\":0-10,\
\"pedals\":[{{\"name\":\"...\",\"knobs\":[0-1,0-1,0-1]}}],\"why\":\"one short sentence\"}}. Use at most 6 pedals in signal order.",
        amps.join(", "),
        cabs.join(", "),
        peds.join(", ")
    )
}

fn fuzzy<'a>(want: &str, names: impl Iterator<Item = (usize, &'a str)>) -> Option<usize> {
    let w = want.to_lowercase();
    let mut best = None;
    let mut score = 0;
    for (i, n) in names {
        let n = n.to_lowercase();
        if n == w {
            return Some(i);
        }
        let s = w.split(|c: char| !c.is_alphanumeric()).filter(|t| t.len() > 1 && n.contains(*t)).count();
        if s > score {
            score = s;
            best = Some(i);
        }
    }
    best
}

/// Turn the LLM's JSON into rig settings; unknown fields keep current values.
pub fn tone_from_json(v: &serde_json::Value, base: &ToneSettings) -> (ToneSettings, String) {
    let mut t = base.clone();
    let num = |k: &str, d: f32| v.get(k).and_then(|x| x.as_f64()).map(|x| (x as f32).clamp(0.0, 10.0)).unwrap_or(d);
    if let Some(a) = v.get("amp").and_then(|x| x.as_str()) {
        if let Some(i) = fuzzy(a, AMPS.iter().enumerate().map(|(i, m)| (i, m.name))) {
            t.amp.model = i;
            t.amp.cab = AMPS[i].default_cab;
        }
    }
    if let Some(c) = v.get("cabinet").and_then(|x| x.as_str()) {
        if let Some(i) = fuzzy(c, CABS.iter().enumerate().map(|(i, m)| (i, m.name))) {
            t.amp.cab = i;
        }
    }
    if let Some(m) = v.get("mic").and_then(|x| x.as_str()) {
        let m = m.to_lowercase();
        t.amp.mic = if m.contains("ribbon") { 2 } else if m.contains("cond") { 1 } else { 0 };
    }
    t.amp.gain = num("gain", t.amp.gain);
    t.amp.bass = num("bass", t.amp.bass);
    t.amp.mids = num("mids", t.amp.mids);
    t.amp.treble = num("treble", t.amp.treble);
    t.amp.presence = num("presence", t.amp.presence);
    t.amp.master = num("master", t.amp.master);
    t.amp.reverb_level = num("reverb", t.amp.reverb_level);
    t.amp.reverb_on = t.amp.reverb_level > 0.2;
    if let Some(ps) = v.get("pedals").and_then(|x| x.as_array()) {
        t.pedals.clear();
        for p in ps.iter().take(dsp::MAX_PEDALS) {
            let name = p.get("name").and_then(|x| x.as_str()).or_else(|| p.as_str()).unwrap_or("");
            if let Some(i) = fuzzy(name, PEDALS.iter().enumerate().map(|(i, m)| (i, m.name)))
                .or_else(|| fuzzy(name, PEDALS.iter().enumerate().map(|(i, m)| (i, m.like))))
            {
                let mut slot = PedalSlot::new(PEDALS[i].kind);
                if let Some(k) = p.get("knobs").and_then(|x| x.as_array()) {
                    for (j, kv) in k.iter().take(3).enumerate() {
                        let x = kv.as_f64().unwrap_or(0.5) as f32;
                        slot.knobs[j] = if x > 1.0 { (x / 10.0).clamp(0.0, 1.0) } else { x.clamp(0.0, 1.0) };
                    }
                }
                t.pedals.push(slot);
            }
        }
    }
    let why = v.get("why").and_then(|x| x.as_str()).unwrap_or("Tone set by the AI.").to_string();
    (t, why)
}

/// Built-in assistant used when no local LLM is running: matches style words.
pub fn tone_from_words(request: &str) -> (ToneSettings, String) {
    let r = request.to_lowercase();
    let has = |ws: &[&str]| ws.iter().any(|w| r.contains(w));
    let pick = |name: &str| PRESETS.iter().find(|p| p.name == name).unwrap();
    let (mut t, why) = if has(&["djent", "metal", "metalcore", "thrash", "chug", "brutal"]) {
        let mut t = apply_preset(pick("Modern Rock Wall"));
        t.amp.gain = 8.5;
        t.amp.mids = 3.5;
        (t, "Tight high-gain amp, gate and a drive pedal as a tightening boost.")
    } else if has(&["jazz", "bossa", "fusion"]) {
        (apply_preset(pick("Warm Jazz")), "Clean, round jazz tone with treble rolled off.")
    } else if has(&["country", "chicken", "twang", "nashville"]) {
        (apply_preset(pick("Country Snap")), "Bright clean amp with compression and a short slap delay.")
    } else if has(&["funk", "rnb", "r&b", "neo soul"]) {
        let mut t = apply_preset(pick("Glass Clean"));
        t.pedals = vec![PedalSlot { kind: PedalKind::Compressor, on: true, knobs: [0.6, 0.2, 0.5] }, PedalSlot { kind: PedalKind::AutoWah, on: true, knobs: [0.6, 0.6, 0.7] }];
        (t, "Glassy clean with squash and an envelope wah for funk.")
    } else if has(&["shoegaze", "ambient", "post-rock", "post rock", "dream", "worship", "atmos"]) {
        let mut t = apply_preset(pick("Glass Clean"));
        t.pedals = vec![
            PedalSlot { kind: PedalKind::GreenDrive, on: true, knobs: [0.4, 0.6, 0.6] },
            PedalSlot { kind: PedalKind::Chorus, on: true, knobs: [0.2, 0.6, 0.4] },
            PedalSlot { kind: PedalKind::TapeEcho, on: true, knobs: [0.55, 0.6, 0.45] },
            PedalSlot { kind: PedalKind::SpringVerb, on: true, knobs: [0.9, 0.5, 0.6] },
        ];
        (t, "Washy edge-of-breakup amp with modulation, long echoes and big reverb.")
    } else if has(&["grunge", "stoner", "doom", "fuzz", "90s"]) {
        let mut t = apply_preset(pick("Vintage Stack Crunch"));
        t.pedals = vec![PedalSlot { kind: PedalKind::TriangleFuzz, on: true, knobs: [0.7, 0.4, 0.6] }];
        (t, "Crunchy stack driven by a big sustaining fuzz.")
    } else if has(&["blues", "srv", "texas", "bb king"]) {
        let mut t = apply_preset(pick("Pushed British"));
        t.amp.model = 0;
        t.amp.cab = 4;
        t.amp.gain = 5.5;
        t.pedals = vec![PedalSlot { kind: PedalKind::GreenDrive, on: true, knobs: [0.35, 0.5, 0.75] }, PedalSlot { kind: PedalKind::SpringVerb, on: true, knobs: [0.5, 0.5, 0.3] }];
        (t, "Tweed-style amp pushed by a green overdrive, with spring reverb.")
    } else if has(&["punk", "pop punk", "garage"]) {
        (apply_preset(pick("High Octane")), "Hot British lead amp, no frills.")
    } else if has(&["lead", "solo", "shred", "sustain"]) {
        (apply_preset(pick("Singing Lead")), "Saturated lead amp with a boost and delay for solos.")
    } else if has(&["classic rock", "rock", "70s", "80s", "hard rock", "ac"]) {
        (apply_preset(pick("Vintage Stack Crunch")), "Classic British stack crunch.")
    } else if has(&["indie", "surf", "chime", "jangle"]) {
        let mut t = apply_preset(pick("Pushed British"));
        t.amp.gain = 4.0;
        t.pedals = vec![PedalSlot { kind: PedalKind::Tremolo, on: true, knobs: [0.4, 0.5, 0.2] }, PedalSlot { kind: PedalKind::SpringVerb, on: true, knobs: [0.7, 0.6, 0.5] }];
        (t, "Chimey British combo with tremolo and spring reverb.")
    } else if has(&["crunch", "rhythm"]) {
        (apply_preset(pick("Rhythm Grind")), "Saturated rhythm crunch.")
    } else {
        (apply_preset(pick("Studio Default")), "A balanced studio starting point; describe a genre or artist style for a closer match.")
    };
    if has(&["dark", "warm"]) {
        t.amp.treble = (t.amp.treble - 2.0).max(0.0);
    }
    if has(&["bright", "sparkle"]) {
        t.amp.treble = (t.amp.treble + 2.0).min(10.0);
    }
    if has(&["delay", "echo"]) && !t.pedals.iter().any(|p| matches!(p.kind, PedalKind::AnalogDelay | PedalKind::TapeEcho)) {
        t.pedals.push(PedalSlot { kind: PedalKind::AnalogDelay, on: true, knobs: [0.4, 0.35, 0.3] });
    }
    (t, why.to_string())
}

/// Listen to a recorded guitar (e.g. the split guitar stem) and guess a matching rig:
/// low crest factor = heavily clipped (high gain), high-frequency share = brightness.
pub fn tone_from_audio(samples: &[f32], sr: f32) -> (ToneSettings, String) {
    let n = samples.len().min((sr * 60.0) as usize);
    let s = &samples[..n];
    if s.is_empty() {
        return (ToneSettings::default(), "No audio to listen to.".into());
    }
    let mut crest_sum = 0.0;
    let mut frames = 0.0;
    let mut hf = 0.0;
    let mut tot = 0.0;
    let mut prev = 0.0;
    for c in s.chunks(2048) {
        let rms = (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt();
        if rms < 0.01 {
            continue;
        }
        let peak = c.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        crest_sum += peak / rms;
        frames += 1.0;
        for v in c {
            let d = v - prev;
            prev = *v;
            hf += d * d;
            tot += v * v;
        }
    }
    if frames < 1.0 {
        return (ToneSettings::default(), "The audio is too quiet to analyse.".into());
    }
    let crest = crest_sum / frames;
    let bright = (hf / tot.max(1e-9)).sqrt();
    let (mut t, kind) = if crest < 3.2 {
        (apply_preset(&PRESETS[8]), "high-gain")
    } else if crest < 4.5 {
        (apply_preset(&PRESETS[4]), "crunchy")
    } else {
        (apply_preset(&PRESETS[0]), "clean")
    };
    t.amp.treble = (3.0 + bright * 25.0).clamp(1.0, 9.5);
    t.amp.presence = (2.5 + bright * 20.0).clamp(1.0, 9.0);
    (t, format!("Heard a {kind} guitar (crest {crest:.1}, brightness {:.0}%). Rig set to match.", (bright * 100.0).min(100.0)))
}

/// Built-in single-note pitch tracker → notes (works without extra installs).
pub fn hear_notes(samples: &[f32], sr: f32, max_secs: f32) -> Vec<HeardNote> {
    let target = 16000.0;
    let x = crate::audio::resample(&samples[..samples.len().min((sr * max_secs) as usize)], sr, target);
    let hop = 320; // 20 ms
    let win = 1024;
    let mut frames: Vec<(Option<i32>, f32)> = vec![];
    let mut i = 0;
    while i + win < x.len() {
        let w = &x[i..i + win];
        let p = dsp::detect_pitch(w, target);
        let rms = (w.iter().map(|v| v * v).sum::<f32>() / win as f32).sqrt();
        let m = p.map(|(f, _)| dsp::freq_to_note(f, 440.0).0).filter(|m| (40..=88).contains(m));
        frames.push((m, rms));
        i += hop;
    }
    let dt = hop as f32 / target;
    let mut out: Vec<HeardNote> = vec![];
    let mut cur: Option<(i32, usize)> = None;
    for (k, (m, rms)) in frames.iter().enumerate() {
        let onset = k > 0 && *rms > frames[k - 1].1 * 1.8 && *rms > 0.02;
        match (cur, m) {
            (Some((cm, start)), Some(mm)) if *mm == cm && !onset => {
                let _ = start;
            }
            _ => {
                if let Some((cm, start)) = cur.take() {
                    if k - start >= 3 {
                        out.push(HeardNote { start: start as f32 * dt, end: k as f32 * dt, midi: cm });
                    }
                }
                if let Some(mm) = m {
                    cur = Some((*mm, k));
                }
            }
        }
    }
    if let Some((cm, start)) = cur {
        if frames.len() - start >= 3 {
            out.push(HeardNote { start: start as f32 * dt, end: frames.len() as f32 * dt, midi: cm });
        }
    }
    out
}

/// Audio → tab notes. Uses Basic Pitch events when given, else the built-in tracker.
pub fn transcribe(events: Option<Vec<(f32, f32, i32)>>, samples: &[f32], sr: f32, tempo: f32, tuning: &[i32; 6]) -> (Vec<TabNote>, String) {
    let (heard, how) = match events {
        Some(ev) if !ev.is_empty() => (ev.into_iter().map(|(s, e, m)| HeardNote { start: s, end: e, midi: m }).collect::<Vec<_>>(), "Basic Pitch (polyphonic)"),
        _ => (hear_notes(samples, sr, 90.0), "built-in pitch tracker (single notes)"),
    };
    let notes = tabs::place_on_fretboard(&heard, tempo, tuning);
    let msg = format!("Recognised {} notes with {how}.", notes.len());
    (notes, msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pluck(midi: i32, secs: f32, sr: f32) -> Vec<f32> {
        let f = 440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0);
        (0..(secs * sr) as usize).map(|i| (i as f32 / sr * f * std::f32::consts::TAU).sin() * 0.5 * (-(i as f32) / sr * 2.0).exp()).collect()
    }

    #[test]
    fn recognizes_single_notes_as_tab() {
        let sr = 44100.0;
        let mut x = vec![];
        for m in [57, 60, 62, 64] {
            x.extend(pluck(m, 0.5, sr));
        }
        let (notes, _) = transcribe(None, &x, sr, 120.0, &tabs::STANDARD);
        let pitches: Vec<i32> = notes.iter().map(|n| n.midi(&tabs::STANDARD)).collect();
        assert_eq!(pitches, vec![57, 60, 62, 64], "{notes:?}");
    }

    #[test]
    fn words_pick_sensible_rigs() {
        let (metal, _) = tone_from_words("tight djent metal rhythm");
        assert!(metal.amp.gain >= 8.0);
        let (jazz, _) = tone_from_words("warm jazz chords");
        assert!(jazz.amp.gain < 4.0);
        let v = serde_json::json!({"amp":"Plexi","cabinet":"4x12 British","gain":7,"pedals":[{"name":"Green Drive","knobs":[0.3,0.5,0.8]}]});
        let (t, _) = tone_from_json(&v, &ToneSettings::default());
        assert_eq!(AMPS[t.amp.model].name, "Plexi Stack");
        assert_eq!(t.pedals[0].kind, PedalKind::GreenDrive);
    }

    #[test]
    fn text_tab_round_trip() {
        let s = tabs::TabSong::demo();
        let again = tabs::parse_ascii(&tabs::to_ascii(&s));
        assert_eq!(s.notes.len(), again.notes.len());
    }

    #[test]
    fn tuner_rules_match_the_original() {
        use crate::dsp;
        // names: midi % 12 into a sharp or flat table; negative -> empty (0x100e047b4 / 0x100e04640)
        assert_eq!(dsp::note_name_in(61, false), "C#4");
        assert_eq!(dsp::note_name_in(61, true), "Db4");
        assert_eq!(dsp::note_name_in(-1, false), "");
        // in tune within +/-3 cents (0x100d8914c)
        let (m, c) = dsp::freq_to_note(440.0 * 2f32.powf(2.9 / 1200.0), 440.0);
        assert_eq!(m, 69);
        assert!(c.round().abs() <= dsp::IN_TUNE_CENTS);
        let (_, c) = dsp::freq_to_note(440.0 * 2f32.powf(4.2 / 1200.0), 440.0);
        assert!(c.round().abs() > dsp::IN_TUNE_CENTS);
        // Dropped D only lowers the low string by a whole step
        let dd = tabs::TUNINGS[2].1;
        assert_eq!(dd[5], tabs::STANDARD[5] - 2);
        assert_eq!(&dd[..5], &tabs::STANDARD[..5]);
    }
}
