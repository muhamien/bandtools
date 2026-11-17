//! Guitar tab model: notes on strings/frets with timing in beats,
//! plain-text tab import/export and note-to-fret placement.

use serde::{Deserialize, Serialize};

/// Standard tuning, string 0 = high e ... string 5 = low E (MIDI numbers).
pub const STANDARD: [i32; 6] = [64, 59, 55, 50, 45, 40];

/// The tuner's tuning presets, named as in the original's string table
/// ("LearnToPlayTunerStrings|||Standard Tuning / Open D / Dropped D / Tune Down Whole Step",
/// __cstring @0x1013e4883..0x1013e4918). Pitches are the usual ones for those names.
pub const TUNINGS: [(&str, [i32; 6]); 4] = [
    ("Standard Tuning", STANDARD),
    ("Open D", [62, 57, 54, 50, 45, 38]),
    ("Dropped D", [64, 59, 55, 50, 45, 38]),
    ("Tune Down Whole Step", [62, 57, 53, 48, 43, 38]),
];
pub const STRING_NAMES: [&str; 6] = ["e", "B", "G", "D", "A", "E"];
pub const MAX_FRET: i32 = 22;
/// One text column in a plain-text tab = a sixteenth note.
pub const STEP: f32 = 0.25;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct TabNote {
    pub string: u8,
    pub fret: u8,
    pub beat: f32,
    pub len: f32,
    #[serde(default)]
    pub tech: Option<char>,
}

impl TabNote {
    pub fn midi(&self, tuning: &[i32; 6]) -> i32 {
        tuning[self.string as usize] + self.fret as i32
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TabSong {
    pub title: String,
    pub artist: String,
    pub tempo: f32,
    pub beats_per_bar: u32,
    pub tuning: [i32; 6],
    pub notes: Vec<TabNote>,
}

impl TabSong {
    pub fn empty() -> Self {
        Self { title: "Untitled".into(), artist: String::new(), tempo: 120.0, beats_per_bar: 4, tuning: STANDARD, notes: vec![] }
    }

    /// An original practice riff (A minor pentatonic) used as the demo.
    pub fn demo() -> Self {
        let text = "\
e|----------------|----------------|----------------|--------5-8-5---|
B|--------------5-|-8-5-----------5|-8-5-8-5-------8|-5-8-------8-5--|
G|------5-7-5-7---|-----7-5-7-----5|---------7-5-7--|----------------|
D|--5-7-----------|----------7-5---|----------------|----------------|
A|-7--------------|----------------|----------------|----------------|
E|----------------|----------------|----------------|----------------|
";
        let mut s = parse_ascii(text);
        s.title = "Pentatonic Warm-up".into();
        s.artist = "Practice riff".into();
        s.tempo = 96.0;
        s
    }

    pub fn total_beats(&self) -> f32 {
        let end = self.notes.iter().map(|n| n.beat + n.len).fold(0.0, f32::max);
        let bar = self.beats_per_bar as f32;
        ((end / bar).ceil().max(1.0)) * bar
    }

    pub fn sort(&mut self) {
        self.notes.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap().then(a.string.cmp(&b.string)));
    }

    pub fn note_at(&self, string: u8, beat: f32) -> Option<usize> {
        self.notes.iter().position(|n| n.string == string && (n.beat - beat).abs() < STEP * 0.5)
    }
}

/// Parse a plain-text tab (six lines starting with string names and `|`).
pub fn parse_ascii(text: &str) -> TabSong {
    let mut song = TabSong::empty();
    let lines: Vec<&str> = text.lines().collect();
    let mut block_start_beat = 0.0f32;
    let mut i = 0;
    while i < lines.len() {
        let is_string = |l: &str| {
            let t = l.trim_start();
            t.len() > 2 && t.contains('|') && t.chars().skip(t.find('|').unwrap_or(0) + 1).all(|c| c != '\u{0}')
                && t[..t.find('|').unwrap_or(0)].len() <= 3
                && t.chars().filter(|c| *c == '-').count() > 2
        };
        if i + 5 < lines.len() && (0..6).all(|k| is_string(lines[i + k])) {
            let mut width = 0usize;
            for k in 0..6 {
                let l = lines[i + k].trim_start();
                let body: Vec<char> = l[l.find('|').unwrap() + 1..].chars().filter(|c| *c != '|').collect();
                width = width.max(body.len());
                let mut col = 0usize;
                while col < body.len() {
                    let c = body[col];
                    if c.is_ascii_digit() {
                        let mut num = c.to_digit(10).unwrap();
                        let mut used = 1;
                        if col + 1 < body.len() && body[col + 1].is_ascii_digit() {
                            let two = num * 10 + body[col + 1].to_digit(10).unwrap();
                            if two as i32 <= MAX_FRET {
                                num = two;
                                used = 2;
                            }
                        }
                        let tech = body.get(col + used).copied().filter(|t| "hpb/\\~".contains(*t));
                        song.notes.push(TabNote { string: k as u8, fret: num as u8, beat: block_start_beat + col as f32 * STEP, len: STEP * 2.0, tech });
                        col += used;
                    } else {
                        col += 1;
                    }
                }
            }
            block_start_beat += width as f32 * STEP;
            let bar = song.beats_per_bar as f32;
            block_start_beat = (block_start_beat / bar).ceil() * bar;
            i += 6;
        } else {
            let l = lines[i].trim();
            if let Some(t) = l.strip_prefix("Title:") {
                song.title = t.trim().into();
            } else if let Some(t) = l.strip_prefix("Artist:") {
                song.artist = t.trim().into();
            } else if let Some(t) = l.strip_prefix("Tempo:") {
                if let Ok(v) = t.trim().parse() {
                    song.tempo = v;
                }
            }
            i += 1;
        }
    }
    // Let each note ring until the next note on the same string.
    song.sort();
    let notes = song.notes.clone();
    for n in song.notes.iter_mut() {
        if let Some(next) = notes.iter().find(|m| m.string == n.string && m.beat > n.beat + 0.01) {
            n.len = (next.beat - n.beat).min(4.0);
        } else {
            n.len = 1.0;
        }
    }
    song
}

/// Write the tab as plain text, 4 bars per line.
pub fn to_ascii(song: &TabSong) -> String {
    let mut out = format!("Title: {}\nArtist: {}\nTempo: {}\n\n", song.title, song.artist, song.tempo);
    let bar = song.beats_per_bar as f32;
    let cols_per_bar = (bar / STEP) as usize;
    let bars = (song.total_beats() / bar) as usize;
    for row in (0..bars).step_by(4) {
        for s in 0..6u8 {
            let mut line = format!("{}|", STRING_NAMES[s as usize]);
            for b in row..(row + 4).min(bars) {
                let mut cells = vec!['-'; cols_per_bar];
                for n in song.notes.iter().filter(|n| n.string == s) {
                    let start = b as f32 * bar;
                    if n.beat >= start - 0.01 && n.beat < start + bar - 0.01 {
                        let c = ((n.beat - start) / STEP).round() as usize;
                        let txt = n.fret.to_string();
                        for (j, ch) in txt.chars().enumerate() {
                            if c + j < cols_per_bar {
                                cells[c + j] = ch;
                            }
                        }
                    }
                }
                line.extend(cells);
                line.push('|');
            }
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

/// A note heard in audio: start/length in seconds and MIDI pitch.
#[derive(Clone, Debug)]
pub struct HeardNote {
    pub start: f32,
    pub end: f32,
    pub midi: i32,
}

/// Place heard notes on strings and frets, preferring small hand movements
/// and low positions, and never two notes on one string at the same time.
pub fn place_on_fretboard(heard: &[HeardNote], tempo: f32, tuning: &[i32; 6]) -> Vec<TabNote> {
    let beats_per_sec = tempo / 60.0;
    let mut out: Vec<TabNote> = vec![];
    let mut hand = 3.0f32;
    let mut sorted = heard.to_vec();
    sorted.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap().then(b.midi.cmp(&a.midi)));
    let mut i = 0;
    while i < sorted.len() {
        // group notes starting together (chords)
        let q = |t: f32| ((t * beats_per_sec) / STEP).round() * STEP;
        let beat = q(sorted[i].start);
        let mut j = i;
        while j < sorted.len() && (q(sorted[j].start) - beat).abs() < 0.01 {
            j += 1;
        }
        let mut used = [false; 6];
        let mut frets = vec![];
        for h in &sorted[i..j] {
            let mut best: Option<(f32, usize, i32)> = None;
            for s in 0..6 {
                if used[s] {
                    continue;
                }
                let f = h.midi - tuning[s];
                if !(0..=MAX_FRET).contains(&f) {
                    continue;
                }
                let cost = if f == 0 { 1.0 } else { (f as f32 - hand).abs() } + f as f32 * 0.08;
                if best.map_or(true, |b| cost < b.0) {
                    best = Some((cost, s, f));
                }
            }
            if let Some((_, s, f)) = best {
                used[s] = true;
                if f > 0 {
                    frets.push(f as f32);
                }
                let len = (((h.end - h.start) * beats_per_sec) / STEP).round().max(1.0) * STEP;
                out.push(TabNote { string: s as u8, fret: f as u8, beat, len, tech: None });
            }
        }
        if !frets.is_empty() {
            let avg = frets.iter().sum::<f32>() / frets.len() as f32;
            hand = hand * 0.4 + avg * 0.6;
        }
        i = j;
    }
    out
}
