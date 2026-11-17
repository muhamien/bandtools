//! The main window: control bar, tone library, tracks, and the bottom
//! editor (amp, pedalboard, guitar tab, stem splitter, AI, tuner).

use crate::ai;
use crate::audio::{self, Audio, TabEvent, Track};
use crate::dsp::{self, PedalSlot, ToneSettings, AMPS, CABS, MICS, PEDALS};
use crate::jobs::{self, Job, JobState};
use crate::tabs::{self, TabNote, TabSong};
use crate::widgets::{self, c};
use egui::{pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind};
use std::time::Instant;

#[derive(PartialEq, Clone, Copy)]
enum Page {
    Amp,
    Pedals,
    Tab,
    Stems,
    Ai,
    Tuner,
}

const BG: Color32 = Color32::from_rgb(30, 30, 32);
const PANEL: Color32 = Color32::from_rgb(43, 43, 46);
const BAR: Color32 = Color32::from_rgb(56, 56, 60);
const ACCENT: Color32 = Color32::from_rgb(255, 160, 50);
const TEXT: Color32 = Color32::from_rgb(225, 225, 228);
const DIM: Color32 = Color32::from_rgb(150, 150, 156);
const STEM_COLORS: [[u8; 3]; 6] = [[240, 150, 50], [220, 90, 120], [90, 200, 120], [80, 150, 240], [170, 120, 230], [140, 140, 150]];

pub struct BandApp {
    audio: Audio,
    tone: ToneSettings,
    page: Page,
    show_library: bool,
    song: TabSong,
    song_dirty: bool,
    cursor: (u8, f32),
    digit_buf: (String, Instant),
    import_text: String,
    show_import: bool,
    tab_muted: bool,
    tab_speed: f32,
    yt_url: String,
    local_file: String,
    stem_model: String,
    stem_job: Job,
    stems_loaded: bool,
    install_job: Job,
    tools: jobs::ToolStatus,
    tools_checked: Instant,
    ai_prompt: String,
    ai_model: String,
    ai_job: Job,
    ai_msg: String,
    ollama_models: Vec<String>,
    tab_job: Job,
    tab_source: usize,
    tab_msg: String,
    a4: f32,
    flats: bool,
    tuner: Option<(i32, f32, f32)>,
    tuner_smooth: f32,
    key: String,
    rec_start: u64,
    take_no: usize,
    status: String,
    shot: Option<String>,
    frames: u32,
    shot_requested: bool,
    started: Instant,
    offscreen: bool,
    requested_at: Option<Instant>,
}

impl BandApp {
    pub fn new(cc: &eframe::CreationContext<'_>, shot: Option<String>) -> Self {
        Self::build(&cc.egui_ctx, shot, false)
    }

    pub fn build(ctx: &egui::Context, shot: Option<String>, offscreen: bool) -> Self {
        let mut vis = egui::Visuals::dark();
        vis.panel_fill = PANEL;
        vis.window_fill = PANEL;
        vis.extreme_bg_color = Color32::from_rgb(22, 22, 24);
        vis.selection.bg_fill = Color32::from_rgb(200, 120, 30);
        vis.widgets.inactive.weak_bg_fill = Color32::from_rgb(64, 64, 68);
        vis.widgets.hovered.weak_bg_fill = Color32::from_rgb(80, 80, 86);
        ctx.set_visuals(vis);
        let tone = ToneSettings::default();
        let audio = Audio::start(&tone, shot.is_none());
        let song = TabSong::demo();
        let mut app = Self {
            audio,
            tone,
            page: Page::Amp,
            show_library: true,
            song,
            song_dirty: true,
            cursor: (0, 0.0),
            digit_buf: (String::new(), Instant::now()),
            import_text: String::new(),
            show_import: false,
            tab_muted: false,
            tab_speed: 1.0,
            yt_url: String::new(),
            local_file: String::new(),
            stem_model: "htdemucs_6s".into(),
            stem_job: Job::default(),
            stems_loaded: false,
            install_job: Job::default(),
            tools: jobs::tool_status(),
            tools_checked: Instant::now(),
            ai_prompt: String::new(),
            ai_model: "qwen2.5:7b".into(),
            ai_job: Job::default(),
            ai_msg: String::new(),
            ollama_models: vec![],
            tab_job: Job::default(),
            tab_source: 0,
            tab_msg: String::new(),
            a4: 440.0,
            flats: false,
            tuner: None,
            tuner_smooth: 0.0,
            key: "A minor".into(),
            rec_start: 0,
            take_no: 1,
            status: String::new(),
            shot,
            frames: 0,
            shot_requested: false,
            started: Instant::now(),
            offscreen,
            requested_at: None,
        };
        app.add_demo_track();
        // Developer aid: open a given page first (used for checking each page).
        app.page = match std::env::var("BANDTOOLS_PAGE").unwrap_or_default().as_str() {
            "pedals" => Page::Pedals,
            "tab" => Page::Tab,
            "stems" => Page::Stems,
            "ai" => Page::Ai,
            "tuner" => Page::Tuner,
            _ => Page::Amp,
        };
        if app.shot.is_none() {
            let models = jobs::ollama_models();
            if let Some(m) = models.first() {
                app.ai_model = m.clone();
            }
            app.ollama_models = models;
        }
        app
    }

    fn sr(&self) -> f32 {
        self.audio.engine.lock().map(|e| e.sr).unwrap_or(48000.0)
    }

    /// Render the demo tab through the rig into an audio track, so there is
    /// something to hear (and to try the tab recognizer on) right away.
    fn add_demo_track(&mut self) {
        let sr = self.sr();
        let secs = self.song.total_beats() * 60.0 / self.song.tempo + 1.5;
        let mut buf = vec![0.0f32; (secs * sr) as usize];
        let mut seed = 777u32;
        for n in &self.song.notes {
            let f = 440.0 * 2f32.powf((n.midi(&self.song.tuning) as f32 - 69.0) / 12.0);
            let start = (n.beat * 60.0 / self.song.tempo * sr) as usize;
            let len = ((n.len * 60.0 / self.song.tempo + 0.3) * sr) as usize;
            let p = (sr / f) as usize;
            let mut ring: Vec<f32> = (0..p.max(2))
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    (seed as f32 / u32::MAX as f32 - 0.5) * 0.6
                })
                .collect();
            for k in 0..len {
                let i = k % ring.len();
                let nxt = ring[(i + 1) % ring.len()];
                let v = ring[i];
                ring[i] = (v + nxt) * 0.498;
                if let Some(o) = buf.get_mut(start + k) {
                    *o += v;
                }
            }
        }
        let mut tone = ai::apply_preset(&ai::PRESETS[7]);
        tone.pedals.retain(|p| p.kind != dsp::PedalKind::AnalogDelay);
        let mut chain = dsp::ToneChain::new(sr, &tone);
        for v in buf.iter_mut() {
            *v = chain.process(*v * 0.8);
        }
        let peak = buf.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
        for v in buf.iter_mut() {
            *v *= 0.8 / peak;
        }
        if let Ok(mut e) = self.audio.engine.lock() {
            e.tracks.push(Track::new("Demo Riff (amped)", buf, [80, 150, 240]));
        }
    }

    fn push_tone(&mut self) {
        if let Ok(mut e) = self.audio.engine.lock() {
            e.chain.apply(&self.tone);
        }
    }

    fn push_song(&mut self) {
        if !self.song_dirty {
            return;
        }
        self.song_dirty = false;
        self.song.sort();
        if let Ok(mut e) = self.audio.engine.lock() {
            let sr = e.sr;
            let tempo = self.song.tempo * self.tab_speed;
            e.tempo = tempo;
            e.beats_per_bar = self.song.beats_per_bar;
            let ev = if self.tab_muted {
                vec![]
            } else {
                self.song
                    .notes
                    .iter()
                    .map(|n| TabEvent { at: (n.beat * 60.0 / tempo * sr) as u64, midi: n.midi(&self.song.tuning), len: (n.len * 60.0 / tempo * sr) as u64 })
                    .collect()
            };
            e.set_tab_events(ev);
        }
    }

    fn beat_now(&self) -> f32 {
        self.audio.engine.lock().map(|e| e.pos as f32 / e.sr * e.tempo / 60.0).unwrap_or(0.0)
    }

    fn toggle_play(&mut self) {
        self.push_song();
        let mut finish_rec = None;
        if let Ok(mut e) = self.audio.engine.lock() {
            e.playing = !e.playing;
            if !e.playing && e.recording {
                e.recording = false;
                finish_rec = Some(std::mem::take(&mut e.rec_buf));
            }
        }
        if let Some(buf) = finish_rec {
            self.finish_take(buf);
        }
    }

    fn finish_take(&mut self, buf: Vec<f32>) {
        if buf.is_empty() {
            self.status = "Nothing was recorded (is the guitar input on?)".into();
            return;
        }
        let mut full = vec![0.0; self.rec_start as usize];
        full.extend(buf);
        let name = format!("Guitar Take {}", self.take_no);
        self.take_no += 1;
        let path = jobs::workspace_dir().join(format!("{name}.wav"));
        let sr = self.sr();
        let _ = audio::save_wav(&path.to_string_lossy(), &full, sr);
        if let Ok(mut e) = self.audio.engine.lock() {
            e.tracks.push(Track::new(&name, full, [240, 150, 50]));
        }
        self.status = format!("Saved {}", path.display());
    }

    fn record(&mut self) {
        if !self.audio.input_on() {
            self.audio.start_input();
        }
        self.push_song();
        if let Ok(mut e) = self.audio.engine.lock() {
            e.rec_buf.clear();
            e.recording = true;
            e.playing = true;
            self.rec_start = e.pos;
        }
    }

    // ------------------------------------------------------------ control bar

    fn control_bar(&mut self, ui: &mut egui::Ui) {
        let (playing, recording, metronome, master, level, pos_beats) = {
            let e = self.audio.engine.lock().unwrap();
            (e.playing, e.recording, e.metronome, e.master, e.level, e.pos as f32 / e.sr * e.tempo / 60.0)
        };
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let lib = if self.show_library { "▤ Library" } else { "▢ Library" };
            if ui.add(egui::Button::new(lib).selected(self.show_library)).on_hover_text("Show the tone library").clicked() {
                self.show_library = !self.show_library;
            }
            ui.add_space(20.0);
            if ui.add(egui::Button::new(RichText::new("⏮").size(18.0)).min_size(vec2(38.0, 30.0))).on_hover_text("Go to beginning").clicked() {
                if let Ok(mut e) = self.audio.engine.lock() {
                    e.seek(0);
                }
            }
            let play = if playing { "⏸" } else { "▶" };
            if ui.add(egui::Button::new(RichText::new(play).size(18.0)).min_size(vec2(44.0, 30.0)).selected(playing)).on_hover_text("Play / stop (Space)").clicked() {
                self.toggle_play();
            }
            let rec = egui::Button::new(RichText::new("⏺").size(18.0).color(if recording { Color32::WHITE } else { Color32::from_rgb(230, 70, 60) }))
                .min_size(vec2(38.0, 30.0))
                .fill(if recording { Color32::from_rgb(200, 40, 40) } else { BAR });
            if ui.add(rec).on_hover_text("Record your guitar through the rig (R)").clicked() {
                if recording {
                    self.toggle_play();
                } else {
                    self.record();
                }
            }
            ui.add_space(14.0);
            // LCD
            let (rect, _) = ui.allocate_exact_size(vec2(420.0, 42.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(rect, 7.0, Color32::from_rgb(18, 20, 24));
            p.rect_stroke(rect, 7.0, Stroke::new(1.0, Color32::from_gray(70)), StrokeKind::Inside);
            let bar = (pos_beats / self.song.beats_per_bar as f32).floor() as i32 + 1;
            let beat = (pos_beats % self.song.beats_per_bar as f32).floor() as i32 + 1;
            let cols = [
                (format!("{bar:>3}.{beat}"), "BAR  BEAT"),
                (format!("{:.0}", self.song.tempo * self.tab_speed), "TEMPO"),
                (format!("{}/4", self.song.beats_per_bar), "TIME"),
                (self.key.clone(), "KEY"),
            ];
            let w = rect.width() / 4.0;
            for (i, (v, l)) in cols.iter().enumerate() {
                let x = rect.left() + w * (i as f32 + 0.5);
                p.text(pos2(x, rect.top() + 17.0), Align2::CENTER_CENTER, v, FontId::monospace(18.0), Color32::from_rgb(235, 238, 245));
                p.text(pos2(x, rect.bottom() - 8.0), Align2::CENTER_CENTER, *l, FontId::proportional(9.0), DIM);
                if i > 0 {
                    p.line_segment([pos2(rect.left() + w * i as f32, rect.top() + 6.0), pos2(rect.left() + w * i as f32, rect.bottom() - 6.0)], Stroke::new(1.0, Color32::from_gray(50)));
                }
            }
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Tempo").small().color(DIM));
                    if ui.add(egui::DragValue::new(&mut self.song.tempo).range(40.0..=240.0).speed(0.5).suffix(" bpm")).changed() {
                        self.song_dirty = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Key").small().color(DIM));
                    egui::ComboBox::from_id_salt("key").width(80.0).selected_text(&self.key).show_ui(ui, |ui| {
                        for k in ["E minor", "A minor", "D minor", "G major", "C major", "D major", "A major", "E major"] {
                            ui.selectable_value(&mut self.key, k.to_string(), k);
                        }
                    });
                });
            });
            let met = egui::Button::new(RichText::new("Metronome").size(13.0)).selected(metronome);
            if ui.add(met).on_hover_text("Metronome").clicked() {
                if let Ok(mut e) = self.audio.engine.lock() {
                    e.metronome = !e.metronome;
                }
            }
            if ui.add(egui::Button::new("♪ Tuner").selected(self.page == Page::Tuner)).clicked() {
                self.page = Page::Tuner;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut m = master;
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    if ui.add(egui::Slider::new(&mut m, 0.0..=1.2).show_value(false).text("Master")).changed() {
                        if let Ok(mut e) = self.audio.engine.lock() {
                            e.master = m;
                        }
                    }
                    widgets::meter(ui, level, 150.0);
                });
                let on = self.audio.input_on();
                let b = egui::Button::new(if on { "Input ON" } else { "Input off" }).selected(on);
                if ui.add(b).on_hover_text(&self.audio.input_status).clicked() {
                    if on {
                        self.audio.stop_input();
                    } else {
                        self.audio.start_input();
                        if let Ok(mut e) = self.audio.engine.lock() {
                            e.monitor = true;
                        }
                    }
                }
            });
        });
    }

    // ------------------------------------------------------------ library

    fn library(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.label(RichText::new("Tone Library").strong().size(15.0));
        ui.label(RichText::new("Guitar amp & pedal presets").small().color(DIM));
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for group in ["Clean", "Crunch", "Distorted"] {
                ui.label(RichText::new(group.to_uppercase()).small().color(ACCENT));
                for p in ai::PRESETS.iter().filter(|p| p.group == group) {
                    let resp = ui.add_sized([ui.available_width(), 24.0], egui::Button::new(RichText::new(p.name)));
                    if resp.clicked() {
                        self.tone = ai::apply_preset(p);
                        self.push_tone();
                        self.status = format!("Loaded preset {}", p.name);
                    }
                }
                ui.add_space(6.0);
            }
            ui.separator();
            ui.label(RichText::new("AI QUICK TONE").small().color(ACCENT));
            ui.add(egui::TextEdit::singleline(&mut self.ai_prompt).hint_text("e.g. 90s grunge rhythm").desired_width(f32::INFINITY));
            if ui.button("Set up my tone").clicked() {
                self.run_ai_tone();
                self.page = Page::Ai;
            }
        });
    }

    // ------------------------------------------------------------ tracks

    fn tracks(&mut self, ui: &mut egui::Ui) {
        let px_per_beat = 26.0;
        let header_w = 210.0;
        let row_h = 64.0;
        let (sr, tempo, pos, n_tracks, longest) = {
            let e = self.audio.engine.lock().unwrap();
            (e.sr, e.tempo, e.pos, e.tracks.len(), e.tracks.iter().map(|t| t.samples.len()).max().unwrap_or(0))
        };
        let total_beats = (longest as f32 / sr * tempo / 60.0).max(self.song.total_beats()).max(32.0) + 8.0;
        let beat_now = pos as f32 / sr * tempo / 60.0;
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let width = header_w + total_beats * px_per_beat;
            let height = 26.0 + row_h * (n_tracks + 1) as f32 + 30.0;
            let (rect, resp) = ui.allocate_exact_size(vec2(width.max(ui.available_width()), height.max(ui.available_height())), Sense::click());
            let lane_x = rect.left() + header_w;
            let bar_beats = self.song.beats_per_bar as f32;
            {
                let p = ui.painter();
                p.rect_filled(rect, 0.0, BG);
                let ruler = Rect::from_min_size(rect.min, vec2(rect.width(), 24.0));
                p.rect_filled(ruler, 0.0, Color32::from_rgb(50, 50, 54));
                let mut b = 0.0;
                while b < total_beats {
                    let x = lane_x + b * px_per_beat;
                    let is_bar = (b % bar_beats) == 0.0;
                    p.line_segment([pos2(x, ruler.bottom() - if is_bar { 12.0 } else { 5.0 }), pos2(x, ruler.bottom())], Stroke::new(1.0, Color32::from_gray(120)));
                    if is_bar {
                        p.text(pos2(x + 3.0, ruler.top() + 7.0), Align2::LEFT_CENTER, format!("{}", (b / bar_beats) as i32 + 1), FontId::proportional(10.0), DIM);
                        p.line_segment([pos2(x, ruler.bottom()), pos2(x, rect.bottom())], Stroke::new(1.0, Color32::from_gray(42)));
                    }
                    b += 1.0;
                }
            }
            let mut y = rect.top() + 26.0;
            let mut seek_to = None;
            // ---- tab track
            {
                let hdr = Rect::from_min_size(pos2(rect.left(), y), vec2(header_w - 2.0, row_h - 4.0));
                ui.painter().rect_filled(hdr, 4.0, Color32::from_rgb(52, 52, 56));
                ui.painter().rect_filled(Rect::from_min_size(hdr.min, vec2(5.0, hdr.height())), 2.0, Color32::from_rgb(110, 200, 120));
                ui.painter().text(hdr.min + vec2(14.0, 14.0), Align2::LEFT_CENTER, "♫ Guitar Tab", FontId::proportional(14.0), TEXT);
                ui.painter().text(hdr.min + vec2(14.0, 30.0), Align2::LEFT_CENTER, &self.song.title, FontId::proportional(11.0), DIM);
                let mrect = Rect::from_min_size(hdr.min + vec2(14.0, 40.0), vec2(22.0, 16.0));
                let mr = ui.interact(mrect, egui::Id::new("tabmute"), Sense::click());
                ui.painter().rect_filled(mrect, 3.0, if self.tab_muted { Color32::from_rgb(60, 140, 230) } else { Color32::from_gray(75) });
                ui.painter().text(mrect.center(), Align2::CENTER_CENTER, "M", FontId::proportional(11.0), TEXT);
                if mr.clicked() {
                    self.tab_muted = !self.tab_muted;
                    self.song_dirty = true;
                    self.push_song();
                }
                let end = self.song.total_beats();
                let reg = Rect::from_min_size(pos2(lane_x, y + 2.0), vec2(end * px_per_beat, row_h - 8.0));
                let rr = ui.interact(reg, egui::Id::new("tabregion"), Sense::click());
                let p = ui.painter();
                p.rect_filled(reg, 5.0, Color32::from_rgb(64, 150, 84));
                p.rect_filled(Rect::from_min_size(reg.min, vec2(reg.width(), 14.0)), 5.0, Color32::from_rgb(84, 180, 104));
                p.text(reg.min + vec2(6.0, 7.0), Align2::LEFT_CENTER, &self.song.title, FontId::proportional(10.0), Color32::from_gray(20));
                for n in &self.song.notes {
                    let x = lane_x + n.beat * px_per_beat;
                    let ny = reg.top() + 18.0 + n.string as f32 * 6.0;
                    p.rect_filled(Rect::from_min_size(pos2(x, ny), vec2((n.len * px_per_beat - 1.0).max(2.0), 4.0)), 1.0, Color32::from_rgb(220, 255, 225));
                }
                if rr.double_clicked() {
                    self.page = Page::Tab;
                }
                y += row_h;
            }
            // ---- audio tracks
            let mut remove = None;
            if let Ok(mut e) = self.audio.engine.lock() {
                let n = e.tracks.len();
                for i in 0..n {
                    let t = &mut e.tracks[i];
                    let hdr = Rect::from_min_size(pos2(rect.left(), y), vec2(header_w - 2.0, row_h - 4.0));
                    ui.painter().rect_filled(hdr, 4.0, Color32::from_rgb(52, 52, 56));
                    ui.painter().rect_filled(Rect::from_min_size(hdr.min, vec2(5.0, hdr.height())), 2.0, c(t.color));
                    ui.painter().text(hdr.min + vec2(14.0, 14.0), Align2::LEFT_CENTER, format!("{}", t.name), FontId::proportional(13.0), TEXT);
                    let mut bx = hdr.min.x + 14.0;
                    for (lbl, on, col) in [("M", t.mute, Color32::from_rgb(60, 140, 230)), ("S", t.solo, Color32::from_rgb(230, 190, 40))] {
                        let r = Rect::from_min_size(pos2(bx, hdr.min.y + 38.0), vec2(22.0, 16.0));
                        let resp = ui.interact(r, egui::Id::new((lbl, i)), Sense::click());
                        ui.painter().rect_filled(r, 3.0, if on { col } else { Color32::from_gray(75) });
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, lbl, FontId::proportional(11.0), if on { Color32::BLACK } else { TEXT });
                        if resp.clicked() {
                            if lbl == "M" {
                                t.mute = !t.mute;
                            } else {
                                t.solo = !t.solo;
                            }
                        }
                        bx += 26.0;
                    }
                    let vr = Rect::from_min_size(pos2(bx + 6.0, hdr.min.y + 42.0), vec2(96.0, 8.0));
                    let vresp = ui.interact(vr.expand(4.0), egui::Id::new(("vol", i)), Sense::click_and_drag());
                    if vresp.dragged() || vresp.clicked() {
                        if let Some(pp) = vresp.interact_pointer_pos() {
                            t.volume = ((pp.x - vr.left()) / vr.width()).clamp(0.0, 1.0);
                        }
                    }
                    ui.painter().rect_filled(vr, 3.0, Color32::from_gray(30));
                    ui.painter().rect_filled(Rect::from_min_size(vr.min, vec2(vr.width() * t.volume, vr.height())), 3.0, Color32::from_gray(170));
                    let xr = Rect::from_min_size(pos2(hdr.right() - 20.0, hdr.top() + 6.0), vec2(16.0, 16.0));
                    if ui.interact(xr, egui::Id::new(("del", i)), Sense::click()).on_hover_text("Remove track").clicked() {
                        remove = Some(i);
                    }
                    ui.painter().text(xr.center(), Align2::CENTER_CENTER, "×", FontId::proportional(11.0), DIM);
                    let len_beats = t.samples.len() as f32 / sr * tempo / 60.0;
                    let reg = Rect::from_min_size(pos2(lane_x, y + 2.0), vec2(len_beats * px_per_beat, row_h - 8.0));
                    let base = c(t.color);
                    let p = ui.painter();
                    p.rect_filled(reg, 5.0, base.gamma_multiply(if t.mute { 0.35 } else { 0.75 }));
                    p.rect_filled(Rect::from_min_size(reg.min, vec2(reg.width(), 14.0)), 5.0, base);
                    p.text(reg.min + vec2(6.0, 7.0), Align2::LEFT_CENTER, &t.name, FontId::proportional(10.0), Color32::from_gray(20));
                    let mid = reg.top() + 14.0 + (reg.height() - 14.0) / 2.0;
                    let hh = (reg.height() - 18.0) / 2.0;
                    let np = t.peaks.len().max(1);
                    let step = (reg.width() / np as f32).max(0.5);
                    for (k, pk) in t.peaks.iter().enumerate() {
                        let x = reg.left() + k as f32 * reg.width() / np as f32;
                        let h = (pk.min(1.0) * hh).max(0.5);
                        p.line_segment([pos2(x, mid - h), pos2(x, mid + h)], Stroke::new(step.min(2.0), Color32::from_black_alpha(130)));
                    }
                    y += row_h;
                }
                if let Some(r) = remove {
                    e.tracks.remove(r);
                }
            }
            ui.painter().text(
                pos2(lane_x + 10.0, y + 18.0),
                Align2::LEFT_CENTER,
                "Drop a WAV file here to add a track  ·  split a YouTube song in the Stem Splitter",
                FontId::proportional(11.0),
                Color32::from_gray(95),
            );
            if resp.clicked() {
                if let Some(pp) = resp.interact_pointer_pos() {
                    if pp.x > lane_x {
                        seek_to = Some(((pp.x - lane_x) / px_per_beat).max(0.0));
                    }
                }
            }
            let px = lane_x + beat_now * px_per_beat;
            ui.painter().line_segment([pos2(px, rect.top()), pos2(px, rect.bottom())], Stroke::new(1.5, Color32::from_rgb(250, 250, 250)));
            ui.painter().add(egui::Shape::convex_polygon(vec![pos2(px - 6.0, rect.top()), pos2(px + 6.0, rect.top()), pos2(px, rect.top() + 9.0)], Color32::WHITE, Stroke::NONE));
            if let Some(b) = seek_to {
                if let Ok(mut e) = self.audio.engine.lock() {
                    let s = (b * 60.0 / e.tempo * e.sr) as u64;
                    e.seek(s);
                }
            }
        });
    }

    // ------------------------------------------------------------ amp page

    fn amp_page(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        let model = &AMPS[self.tone.amp.model];
        ui.horizontal(|ui| {
            ui.label(RichText::new("Amp").strong());
            egui::ComboBox::from_id_salt("amp").width(170.0).selected_text(model.name).show_ui(ui, |ui| {
                for (i, m) in AMPS.iter().enumerate() {
                    if ui.selectable_label(self.tone.amp.model == i, format!("{}  —  {}", m.name, m.style)).clicked() {
                        self.tone.amp.model = i;
                        self.tone.amp.cab = m.default_cab;
                        changed = true;
                    }
                }
            });
            ui.label(RichText::new("Cabinet").strong());
            egui::ComboBox::from_id_salt("cab").width(170.0).selected_text(CABS[self.tone.amp.cab].name).show_ui(ui, |ui| {
                for (i, cb) in CABS.iter().enumerate() {
                    changed |= ui.selectable_value(&mut self.tone.amp.cab, i, cb.name).changed();
                }
            });
            ui.label(RichText::new("Mic").strong());
            egui::ComboBox::from_id_salt("mic").width(140.0).selected_text(MICS[self.tone.amp.mic]).show_ui(ui, |ui| {
                for (i, m) in MICS.iter().enumerate() {
                    changed |= ui.selectable_value(&mut self.tone.amp.mic, i, *m).changed();
                }
            });
            ui.label("Mic position");
            changed |= ui.add(egui::Slider::new(&mut self.tone.amp.mic_edge, 0.0..=1.0).show_value(false).text("center to edge")).changed();
            changed |= ui.checkbox(&mut self.tone.amp_on, "Amp on").changed();
        });
        ui.add_space(6.0);
        let avail = ui.available_width().min(1100.0);
        let (rect, _) = ui.allocate_exact_size(vec2(avail, 190.0), Sense::hover());
        {
            let p = ui.painter();
            p.rect_filled(rect.translate(vec2(0.0, 4.0)), 12.0, Color32::from_black_alpha(140));
            p.rect_filled(rect, 12.0, Color32::from_rgb(28, 26, 24));
            let face = rect.shrink2(vec2(14.0, 12.0));
            let face = Rect::from_min_size(face.min, vec2(face.width(), 120.0));
            p.rect_filled(face, 6.0, Color32::from_rgb(196, 170, 120));
            p.rect_stroke(face, 6.0, Stroke::new(2.0, Color32::from_rgb(120, 100, 60)), StrokeKind::Inside);
            let grille = Rect::from_min_max(pos2(rect.left() + 14.0, face.bottom() + 8.0), pos2(rect.right() - 14.0, rect.bottom() - 10.0));
            p.rect_filled(grille, 4.0, Color32::from_rgb(52, 46, 40));
            let mut gx = grille.left();
            while gx < grille.right() - 12.0 {
                p.line_segment([pos2(gx, grille.top()), pos2(gx + 12.0, grille.bottom())], Stroke::new(1.0, Color32::from_rgb(64, 58, 50)));
                gx += 5.0;
            }
            p.text(grille.center(), Align2::CENTER_CENTER, model.name.to_uppercase(), FontId::proportional(16.0), Color32::from_rgb(220, 200, 150));
        }
        let face = Rect::from_min_size(rect.min + vec2(24.0, 16.0), vec2(rect.width() - 48.0, 112.0));
        let mut kui = ui.new_child(egui::UiBuilder::new().max_rect(face).layout(egui::Layout::left_to_right(egui::Align::Min)));
        kui.spacing_mut().item_spacing.x = 10.0;
        let cap = Color32::from_rgb(40, 36, 30);
        let ink = Color32::from_rgb(40, 30, 20);
        let a = &mut self.tone.amp;
        for (v, l, d) in [(&mut a.gain, "GAIN", 4.5), (&mut a.bass, "BASS", 5.0), (&mut a.mids, "MIDS", 5.0), (&mut a.treble, "TREBLE", 5.0), (&mut a.presence, "PRESENCE", 5.0), (&mut a.master, "MASTER", 7.0)] {
            changed |= widgets::knob(&mut kui, v, 0.0, 10.0, d, l, 52.0, cap, ink).changed();
        }
        kui.add_space(18.0);
        changed |= widgets::knob(&mut kui, &mut a.reverb_level, 0.0, 10.0, 2.2, "REVERB", 44.0, cap, ink).changed();
        changed |= widgets::knob(&mut kui, &mut a.output_db, -24.0, 12.0, -7.0, "OUTPUT dB", 44.0, cap, ink).changed();
        kui.vertical(|ui| {
            ui.add_space(20.0);
            changed |= ui.checkbox(&mut a.reverb_on, RichText::new("Reverb").color(ink)).changed();
        });
        ui.label(RichText::new(format!("{} · {}  —  drag knobs up/down, double-click to reset", model.style, CABS[self.tone.amp.cab].name)).small().color(DIM));
        if changed {
            self.push_tone();
        }
    }

    // ------------------------------------------------------------ pedals page

    fn pedals_page(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Pedalboard").strong());
            ui.label(RichText::new(format!("{} / {} slots · signal flows left to right into the amp", self.tone.pedals.len(), dsp::MAX_PEDALS)).small().color(DIM));
            ui.menu_button("+ Add pedal", |ui| {
                for cat in ["Dynamics", "Drive", "Pitch", "Filter", "Modulation", "Time"] {
                    ui.label(RichText::new(cat).small().color(ACCENT));
                    for p in PEDALS.iter().filter(|p| p.category == cat) {
                        if ui.button(format!("{}  ({})", p.name, p.like)).clicked() && self.tone.pedals.len() < dsp::MAX_PEDALS {
                            self.tone.pedals.push(PedalSlot::new(p.kind));
                            changed = true;
                            ui.close();
                        }
                    }
                }
            });
            if ui.button("Clear board").clicked() {
                self.tone.pedals.clear();
                changed = true;
            }
        });
        ui.add_space(4.0);
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let mut mv: Option<(usize, i32)> = None;
                let mut rm = None;
                let n = self.tone.pedals.len();
                for i in 0..n {
                    let act = widgets::pedal(ui, &mut self.tone.pedals[i], i);
                    changed |= act.changed;
                    if act.remove {
                        rm = Some(i);
                    }
                    if act.left && i > 0 {
                        mv = Some((i, -1));
                    }
                    if act.right && i + 1 < n {
                        mv = Some((i, 1));
                    }
                }
                if n == 0 {
                    ui.label(RichText::new("No pedals yet — use “+ Add pedal” or the AI tone setup.").color(DIM));
                }
                let (r, _) = ui.allocate_exact_size(vec2(110.0, 210.0), Sense::hover());
                ui.painter().rect_filled(r, 10.0, Color32::from_rgb(28, 26, 24));
                ui.painter().rect_filled(Rect::from_min_size(r.min + vec2(8.0, 10.0), vec2(94.0, 40.0)), 4.0, Color32::from_rgb(196, 170, 120));
                ui.painter().text(r.center(), Align2::CENTER_CENTER, format!("AMP\n{}", AMPS[self.tone.amp.model].name), FontId::proportional(12.0), Color32::from_rgb(220, 200, 150));
                if let Some(i) = rm {
                    self.tone.pedals.remove(i);
                    changed = true;
                }
                if let Some((i, d)) = mv {
                    self.tone.pedals.swap(i, (i as i32 + d) as usize);
                    changed = true;
                }
            });
        });
        if changed {
            self.push_tone();
        }
    }

    // ------------------------------------------------------------ tab page

    fn tab_page(&mut self, ui: &mut egui::Ui) {
        let playing = self.audio.engine.lock().map(|e| e.playing).unwrap_or(false);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.song.title).desired_width(180.0).font(FontId::proportional(15.0)));
            ui.add(egui::TextEdit::singleline(&mut self.song.artist).desired_width(130.0).hint_text("artist"));
            if ui.add(egui::Button::new(if playing { "⏸ Stop" } else { "▶ Play tab" }).selected(playing)).clicked() {
                self.toggle_play();
            }
            ui.label("Speed");
            if ui.add(egui::Slider::new(&mut self.tab_speed, 0.25..=1.5).custom_formatter(|v, _| format!("{:.0}%", v * 100.0))).changed() {
                self.song_dirty = true;
            }
            if ui.button("Copy as text").clicked() {
                ui.ctx().copy_text(tabs::to_ascii(&self.song));
                self.status = "Tab copied as text".into();
            }
            if ui.button("Paste / import text tab").clicked() {
                self.show_import = !self.show_import;
            }
            if ui.button("Demo").clicked() {
                self.song = TabSong::demo();
                self.song_dirty = true;
            }
            if ui.button("New").clicked() {
                self.song = TabSong::empty();
                self.song_dirty = true;
            }
        });
        if self.show_import {
            ui.horizontal(|ui| {
                let w = ui.available_width() - 120.0;
                ui.add(egui::TextEdit::multiline(&mut self.import_text).hint_text("Paste a text tab here (lines like  e|---0---3---| )").desired_rows(6).desired_width(w).font(FontId::monospace(11.0)));
                if ui.button("Import").clicked() {
                    let mut s = tabs::parse_ascii(&self.import_text);
                    if s.notes.is_empty() {
                        self.status = "No tab lines found in the pasted text".into();
                    } else {
                        if s.title == "Untitled" {
                            s.title = "Imported tab".into();
                        }
                        self.status = format!("Imported {} notes", s.notes.len());
                        self.song = s;
                        self.song_dirty = true;
                        self.show_import = false;
                    }
                }
            });
        }
        ui.label(RichText::new("Click a string to place the cursor, type fret numbers, Backspace deletes, arrow keys move. Space plays.").small().color(DIM));
        let beat_now = if playing { Some(self.beat_now()) } else { None };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            self.draw_tab(ui, beat_now);
        });
        self.tab_keys(ui);
    }

    fn draw_tab(&mut self, ui: &mut egui::Ui, beat_now: Option<f32>) {
        let width = ui.available_width();
        let bar = self.song.beats_per_bar as f32;
        let bar_w = 250.0;
        let left_pad = 34.0;
        let bars_per_row = (((width - left_pad - 10.0) / bar_w).floor() as usize).max(1);
        let total_bars = ((self.song.total_beats() / bar) as usize + 1).max(4);
        let rows = total_bars.div_ceil(bars_per_row);
        let line_gap = 13.0;
        let row_h = line_gap * 5.0 + 50.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(width, rows as f32 * row_h + 10.0), Sense::click());
        let px_beat = bar_w / bar;
        let paper = Color32::from_rgb(250, 249, 245);
        let p = ui.painter();
        p.rect_filled(rect, 6.0, paper);
        let pos_of = |beat: f32| -> (usize, f32) {
            let b = (beat / bar).floor() as usize;
            let row = b / bars_per_row;
            let col = b % bars_per_row;
            (row, rect.left() + left_pad + col as f32 * bar_w + (beat - b as f32 * bar) * px_beat)
        };
        let ink = Color32::from_gray(60);
        for row in 0..rows {
            let top = rect.top() + 26.0 + row as f32 * row_h;
            let x0 = rect.left() + left_pad;
            let bars_here = (total_bars - row * bars_per_row).min(bars_per_row);
            let x1 = x0 + bars_here as f32 * bar_w;
            for s in 0..6 {
                let y = top + s as f32 * line_gap;
                p.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, Color32::from_gray(150)));
            }
            for (k, ch) in ["T", "A", "B"].iter().enumerate() {
                p.text(pos2(rect.left() + 16.0, top + 12.0 + k as f32 * 15.0), Align2::CENTER_CENTER, *ch, FontId::proportional(12.0), ink);
            }
            for b in 0..=bars_here {
                let x = x0 + b as f32 * bar_w;
                p.line_segment([pos2(x, top), pos2(x, top + line_gap * 5.0)], Stroke::new(1.2, ink));
                if b < bars_here {
                    p.text(pos2(x + 3.0, top - 10.0), Align2::LEFT_CENTER, format!("{}", row * bars_per_row + b + 1), FontId::proportional(10.0), Color32::from_rgb(200, 90, 40));
                }
            }
        }
        let (crow, cx) = pos_of(self.cursor.1);
        let ctop = rect.top() + 26.0 + crow as f32 * row_h + self.cursor.0 as f32 * line_gap;
        p.rect_filled(Rect::from_center_size(pos2(cx + 4.0, ctop), vec2(16.0, 13.0)), 2.0, Color32::from_rgba_unmultiplied(255, 160, 50, 110));
        for n in &self.song.notes {
            let (row, x) = pos_of(n.beat);
            let y = rect.top() + 26.0 + row as f32 * row_h + n.string as f32 * line_gap;
            let active = beat_now.is_some_and(|b| b >= n.beat && b < n.beat + n.len.max(0.25));
            let txt = match n.tech {
                Some(t) => format!("{}{}", n.fret, t),
                None => n.fret.to_string(),
            };
            let w = 7.0 * txt.len() as f32 + 3.0;
            p.rect_filled(Rect::from_center_size(pos2(x + 4.0, y), vec2(w, 12.0)), 2.0, if active { Color32::from_rgb(255, 200, 80) } else { paper });
            p.text(pos2(x + 4.0, y), Align2::CENTER_CENTER, txt, FontId::monospace(12.0), if active { Color32::BLACK } else { Color32::from_gray(20) });
            let stem_top = rect.top() + 26.0 + row as f32 * row_h + line_gap * 5.0 + 6.0;
            p.line_segment([pos2(x + 4.0, stem_top), pos2(x + 4.0, stem_top + if n.len >= 1.0 { 10.0 } else { 16.0 })], Stroke::new(1.0, Color32::from_gray(120)));
        }
        if let Some(b) = beat_now {
            let (row, x) = pos_of(b);
            let top = rect.top() + 18.0 + row as f32 * row_h;
            p.line_segment([pos2(x + 4.0, top), pos2(x + 4.0, top + line_gap * 5.0 + 16.0)], Stroke::new(2.0, Color32::from_rgb(230, 60, 50)));
        }
        if resp.clicked() {
            if let Some(pp) = resp.interact_pointer_pos() {
                let row = ((pp.y - rect.top() - 20.0) / row_h).floor().max(0.0) as usize;
                let top = rect.top() + 26.0 + row as f32 * row_h;
                let s = ((pp.y - top) / line_gap).round().clamp(0.0, 5.0) as u8;
                let col = ((pp.x - rect.left() - left_pad) / bar_w).floor().clamp(0.0, bars_per_row as f32 - 1.0) as usize;
                let inbar = ((pp.x - rect.left() - left_pad - col as f32 * bar_w) / px_beat / tabs::STEP).floor().max(0.0) * tabs::STEP;
                let beat = (row * bars_per_row + col) as f32 * bar + inbar.min(bar - tabs::STEP);
                self.cursor = (s, beat);
            }
        }
    }

    fn tab_keys(&mut self, ui: &mut egui::Ui) {
        if ui.ctx().memory(|m| m.focused().is_some()) {
            return;
        }
        let events = ui.input(|i| i.events.clone());
        for ev in events {
            match ev {
                egui::Event::Text(t) => {
                    for ch in t.chars().filter(|c| c.is_ascii_digit()) {
                        let now = Instant::now();
                        if now.duration_since(self.digit_buf.1).as_secs_f32() > 0.8 {
                            self.digit_buf.0.clear();
                        }
                        self.digit_buf.0.push(ch);
                        self.digit_buf.1 = now;
                        let mut fret: i32 = self.digit_buf.0.parse().unwrap_or(0);
                        if fret > tabs::MAX_FRET {
                            self.digit_buf.0 = ch.to_string();
                            fret = ch.to_digit(10).unwrap_or(0) as i32;
                        }
                        let (s, b) = self.cursor;
                        if let Some(i) = self.song.note_at(s, b) {
                            self.song.notes[i].fret = fret as u8;
                        } else {
                            self.song.notes.push(TabNote { string: s, fret: fret as u8, beat: b, len: 0.5, tech: None });
                        }
                        self.song_dirty = true;
                    }
                }
                egui::Event::Key { key, pressed: true, .. } => {
                    let (s, b) = self.cursor;
                    match key {
                        egui::Key::Backspace | egui::Key::Delete => {
                            if let Some(i) = self.song.note_at(s, b) {
                                self.song.notes.remove(i);
                                self.song_dirty = true;
                            }
                        }
                        egui::Key::ArrowRight => self.cursor.1 = b + tabs::STEP * 2.0,
                        egui::Key::ArrowLeft => self.cursor.1 = (b - tabs::STEP * 2.0).max(0.0),
                        egui::Key::ArrowUp => self.cursor.0 = s.saturating_sub(1),
                        egui::Key::ArrowDown => self.cursor.0 = (s + 1).min(5),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    // ------------------------------------------------------------ stems page

    fn stems_page(&mut self, ui: &mut egui::Ui) {
        if self.tools_checked.elapsed().as_secs() > 5 {
            self.tools = jobs::tool_status();
            self.tools_checked = Instant::now();
        }
        let state = self.stem_job.state();
        let install_state = self.install_job.state();
        let mut start_split = None;
        let mut start_install = false;
        let mut practice = None;
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            ui.label(RichText::new("Split a song into stems").strong().size(15.0));
            ui.label(RichText::new("Paste a YouTube link (or a local audio file), then split it into guitar, vocals, drums, bass, piano and other.").small().color(DIM));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("YouTube");
                let w = ui.available_width();
                ui.add(egui::TextEdit::singleline(&mut self.yt_url).hint_text("https://www.youtube.com/watch?v=…").desired_width(w));
            });
            ui.horizontal(|ui| {
                ui.label("or file ");
                let w = ui.available_width();
                ui.add(egui::TextEdit::singleline(&mut self.local_file).hint_text("drop an audio file on the window").desired_width(w));
            });
            ui.horizontal(|ui| {
                ui.label("Model");
                egui::ComboBox::from_id_salt("stemmodel").selected_text(&self.stem_model).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.stem_model, "htdemucs_6s".to_string(), "htdemucs_6s — 6 stems incl. guitar");
                    ui.selectable_value(&mut self.stem_model, "htdemucs".to_string(), "htdemucs — 4 stems (faster)");
                    ui.selectable_value(&mut self.stem_model, "htdemucs_ft".to_string(), "htdemucs_ft — 4 stems, best quality");
                });
                let busy = state == JobState::Running;
                let src = if !self.yt_url.trim().is_empty() { self.yt_url.trim().to_string() } else { self.local_file.trim().to_string() };
                if ui.add_enabled(!busy && !src.is_empty(), egui::Button::new(RichText::new("Split stems").strong()).fill(Color32::from_rgb(180, 100, 30))).clicked() {
                    start_split = Some(src);
                }
                if busy {
                    ui.spinner();
                }
            });
            if self.stems_loaded {
                ui.horizontal(|ui| {
                    if ui.button("Practice: mute the guitar stem").clicked() {
                        practice = Some(true);
                    }
                    if ui.button("Solo guitar stem").clicked() {
                        practice = Some(false);
                    }
                });
            }
            let ui = &mut cols[1];
            ui.label(RichText::new("Free helper tools").strong());
            let t = &self.tools;
            for (ok, name, what) in [
                (t.ytdlp, "yt-dlp", "downloads YouTube audio"),
                (t.ffmpeg, "ffmpeg", "converts audio"),
                (t.demucs, "Demucs", "splits stems (Meta, MIT license)"),
                (t.basic_pitch, "Basic Pitch", "AI tab recognition (Spotify, Apache-2.0)"),
                (t.ollama, "Ollama", "runs open-source LLMs for AI tone setup"),
            ] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if ok { "✔" } else { "✖" }).color(if ok { Color32::from_rgb(90, 200, 100) } else { Color32::from_rgb(220, 90, 80) }));
                    ui.label(RichText::new(name).strong());
                    ui.label(RichText::new(what).small().color(DIM));
                });
            }
            let ib = install_state == JobState::Running;
            if ui.add_enabled(!ib, egui::Button::new("Install / update helpers")).on_hover_text("Installs or updates yt-dlp, ffmpeg and Ollama with Homebrew, and Demucs + Basic Pitch with pip").clicked() {
                start_install = true;
            }
        });
        if let Some(src) = start_split {
            self.stems_loaded = false;
            let sr = self.sr();
            jobs::split_stems(self.stem_job.clone(), src, self.stem_model.clone(), sr);
        }
        if start_install {
            jobs::install_helpers(self.install_job.clone());
        }
        if let Some(mute) = practice {
            if let Ok(mut e) = self.audio.engine.lock() {
                for t in e.tracks.iter_mut() {
                    let g = t.name.starts_with("Guitar stem");
                    if mute {
                        t.mute = g;
                    } else {
                        t.solo = g;
                    }
                }
            }
        }
        ui.separator();
        let mut log: Vec<String> = self.stem_job.inner.lock().map(|i| i.log.clone()).unwrap_or_default();
        log.extend(self.install_job.inner.lock().map(|i| i.log.clone()).unwrap_or_default());
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            if log.is_empty() {
                ui.label(RichText::new("Progress will show here.").small().color(DIM));
            }
            let skip = log.len().saturating_sub(200);
            for l in log.iter().skip(skip) {
                ui.label(RichText::new(l).monospace().small());
            }
        });
    }

    /// Put the stems a finished split decoded into the timeline (runs on every page).
    fn load_stems(&mut self) {
        let stems = self.stem_job.inner.lock().map(|mut i| std::mem::take(&mut i.stems)).unwrap_or_default();
        let added = stems.len();
        if let Ok(mut e) = self.audio.engine.lock() {
            // Earlier stems are replaced, and the demo/other tracks are muted so the song is heard.
            e.tracks.retain(|t| !t.name.ends_with(" stem"));
            for t in e.tracks.iter_mut() {
                t.mute = true;
            }
            for (i, (name, s)) in stems.into_iter().enumerate() {
                let label = format!("{} stem", capitalize(&name));
                e.tracks.push(Track::new(&label, s, STEM_COLORS[i % STEM_COLORS.len()]));
            }
            e.seek(0);
        }
        self.stems_loaded = true;
        self.tab_source = self.audio.engine.lock().map(|e| e.tracks.iter().position(|t| t.name == "Guitar stem").unwrap_or(0)).unwrap_or(0);
        self.status = format!("Added {added} stem tracks to the timeline");
    }

    // ------------------------------------------------------------ AI page

    fn run_ai_tone(&mut self) {
        let prompt = self.ai_prompt.trim().to_string();
        if prompt.is_empty() {
            self.ai_msg = "Describe the sound you want first (genre, song style, artist style).".into();
            return;
        }
        let job = self.ai_job.clone();
        job.reset();
        let model = self.ai_model.clone();
        let base = self.tone.clone();
        self.ai_msg = format!("Asking {model}…");
        std::thread::spawn(move || {
            let asked = jobs::ensure_ollama(&job).and_then(|_| jobs::ensure_model(&job, &model)).and_then(|_| {
                job.log(format!("Asking {model} for a rig…"));
                jobs::ollama_json(&model, &ai::tone_prompt(&prompt))
            });
            let (tone, why) = match asked {
                Ok(v) => {
                    let (t, w) = ai::tone_from_json(&v, &base);
                    (t, format!("{model}: {w}"))
                }
                Err(e) => {
                    let (t, w) = ai::tone_from_words(&prompt);
                    (t, format!("Built-in assistant (the local AI was not available: {e}): {w}"))
                }
            };
            if let Ok(mut i) = job.inner.lock() {
                i.result = Some(serde_json::json!({ "tone": tone, "why": why }).to_string());
                i.state = JobState::Done;
            }
        });
    }

    fn poll_jobs(&mut self) {
        if self.stem_job.state() == JobState::Done && !self.stems_loaded {
            self.load_stems();
        }
        let res = self.ai_job.inner.lock().ok().and_then(|mut i| i.result.take());
        if let Some(r) = res {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&r) {
                if let Ok(t) = serde_json::from_value::<ToneSettings>(v["tone"].clone()) {
                    self.tone = t;
                    self.push_tone();
                }
                self.ai_msg = v["why"].as_str().unwrap_or("").to_string();
                self.status = "AI tone applied to the amp and pedalboard".into();
            }
            self.ollama_models = jobs::ollama_models();
        }
        let res = self.tab_job.inner.lock().ok().and_then(|mut i| i.result.take());
        if let Some(r) = res {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&r) {
                if let Ok(notes) = serde_json::from_value::<Vec<TabNote>>(v["notes"].clone()) {
                    if !notes.is_empty() {
                        self.song.notes = notes;
                        self.song.title = v["title"].as_str().unwrap_or("Recognized tab").to_string();
                        self.song_dirty = true;
                    }
                }
                self.tab_msg = v["msg"].as_str().unwrap_or("").to_string();
            }
        }
    }

    fn run_tab_ai(&mut self) {
        let (samples, sr, name) = {
            let e = self.audio.engine.lock().unwrap();
            match e.tracks.get(self.tab_source) {
                Some(t) => (t.samples.clone(), e.sr, t.name.clone()),
                None => {
                    drop(e);
                    self.tab_msg = "Add a track first (split a song or record a take).".into();
                    return;
                }
            }
        };
        let tempo = self.song.tempo;
        let tuning = self.song.tuning;
        let job = self.tab_job.clone();
        job.reset();
        self.tab_msg = format!("Listening to “{name}”…");
        std::thread::spawn(move || {
            let mut events = None;
            if jobs::find_tool("basic-pitch").is_some() {
                let path = jobs::workspace_dir().join("tab-source.wav");
                if audio::save_wav(&path.to_string_lossy(), &samples, sr).is_ok() {
                    events = jobs::basic_pitch(&job, &path);
                }
            }
            let (notes, msg) = ai::transcribe(events, &samples, sr, tempo, &tuning);
            if let Ok(mut i) = job.inner.lock() {
                i.result = Some(serde_json::json!({ "notes": notes, "msg": msg, "title": format!("Tab of {name}") }).to_string());
                i.state = JobState::Done;
            }
        });
    }

    fn ai_page(&mut self, ui: &mut egui::Ui) {
        let busy_tone = self.ai_job.state() == JobState::Running;
        let busy_tab = self.tab_job.state() == JobState::Running;
        let names: Vec<String> = self.audio.engine.lock().map(|e| e.tracks.iter().map(|t| t.name.clone()).collect()).unwrap_or_default();
        if self.tab_source >= names.len() {
            self.tab_source = 0;
        }
        let mut do_tone = false;
        let mut do_match = false;
        let mut do_tab = false;
        let mut open_tab = false;
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            ui.label(RichText::new("AI Tone Setup").strong().size(15.0));
            ui.label(RichText::new("Describe a sound; a local open-source LLM picks the amp, cabinet, mic, knobs and pedals.").small().color(DIM));
            ui.add(egui::TextEdit::multiline(&mut self.ai_prompt).hint_text("e.g. warm bluesy lead with a bit of delay, like a 70s live record").desired_rows(2).desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                ui.label("Model");
                egui::ComboBox::from_id_salt("llm").width(150.0).selected_text(&self.ai_model).show_ui(ui, |ui| {
                    let mut list: Vec<String> = self.ollama_models.clone();
                    for (m, use_, _) in ai::MODEL_GUIDE {
                        if use_.starts_with("Tone") && !list.iter().any(|n| n == m) {
                            list.push(m.to_string());
                        }
                    }
                    for n in list {
                        let label = if self.ollama_models.contains(&n) { format!("{n}  ✔") } else { n.clone() };
                        ui.selectable_value(&mut self.ai_model, n, label);
                    }
                });
                if ui.add_enabled(!busy_tone, egui::Button::new(RichText::new("Set up my tone").strong()).fill(Color32::from_rgb(180, 100, 30))).clicked() {
                    do_tone = true;
                }
                if busy_tone {
                    ui.spinner();
                }
            });
            if busy_tone {
                let last = self.ai_job.inner.lock().ok().and_then(|i| i.log.last().cloned()).unwrap_or_else(|| "Working…".into());
                ui.label(RichText::new(last).small().monospace().color(DIM));
            }
            ui.label(
                RichText::new(if self.ollama_models.is_empty() {
                    "Local AI: the first run installs/starts Ollama and downloads the model (a few GB)."
                } else {
                    "Local AI ready (Ollama). Models marked ✔ are downloaded."
                })
                .small()
                .color(DIM),
            );
            if ui.add_enabled(!names.is_empty(), egui::Button::new("Match the tone of the selected track")).on_hover_text("Listens to the track chosen on the right (e.g. the guitar stem) and sets a similar rig").clicked() {
                do_match = true;
            }
            if !self.ai_msg.is_empty() {
                ui.label(RichText::new(&self.ai_msg).color(ACCENT));
            }
            let ui = &mut cols[1];
            ui.label(RichText::new("AI Tab Recognizer").strong().size(15.0));
            ui.label(RichText::new("Listens to a track (best: the split guitar stem) and writes the notes as tab on strings and frets.").small().color(DIM));
            ui.horizontal(|ui| {
                ui.label("Listen to");
                egui::ComboBox::from_id_salt("tabsrc").width(200.0).selected_text(names.get(self.tab_source).cloned().unwrap_or_else(|| "no tracks".into())).show_ui(ui, |ui| {
                    for (i, n) in names.iter().enumerate() {
                        ui.selectable_value(&mut self.tab_source, i, n);
                    }
                });
                if ui.add_enabled(!busy_tab && !names.is_empty(), egui::Button::new(RichText::new("Recognize tab").strong()).fill(Color32::from_rgb(60, 140, 80))).clicked() {
                    do_tab = true;
                }
                if busy_tab {
                    ui.spinner();
                }
            });
            ui.label(
                RichText::new(if self.tools.basic_pitch { "Engine: Basic Pitch (hears chords)" } else { "Engine: built-in pitch tracker (single notes). Install helpers for chord recognition." })
                    .small()
                    .color(DIM),
            );
            if !self.tab_msg.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.tab_msg).color(Color32::from_rgb(120, 210, 130)));
                    if ui.button("Open tab").clicked() {
                        open_tab = true;
                    }
                });
            }
        });
        if do_tone {
            self.run_ai_tone();
        }
        if do_match {
            let (s, sr) = {
                let e = self.audio.engine.lock().unwrap();
                (e.tracks.get(self.tab_source).map(|t| t.samples.clone()), e.sr)
            };
            if let Some(s) = s {
                let (t, why) = ai::tone_from_audio(&s, sr);
                self.tone = t;
                self.push_tone();
                self.ai_msg = why;
            }
        }
        if do_tab {
            self.run_tab_ai();
        }
        if open_tab {
            self.page = Page::Tab;
        }
        ui.separator();
        ui.label(RichText::new("Open-source models behind the AI features (they run on this Mac; nothing is uploaded)").strong());
        egui::Grid::new("models").striped(true).spacing(vec2(18.0, 4.0)).show(ui, |ui| {
            for (m, use_, desc) in ai::MODEL_GUIDE {
                ui.label(RichText::new(*m).monospace());
                ui.label(RichText::new(*use_).color(ACCENT));
                ui.label(RichText::new(*desc).small());
                ui.end_row();
            }
        });
        ui.label(RichText::new("Pick a model and press 'Set up my tone': the app starts Ollama and downloads the model the first time. If the local AI can't run, the built-in style matcher is used.").small().color(DIM));
    }

    // ------------------------------------------------------------ tuner page

    fn tuner_page(&mut self, ui: &mut egui::Ui) {
        let mut turn_on = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Tuner").strong().size(15.0));
            if !self.audio.input_on() && ui.button("Turn on guitar input").clicked() {
                turn_on = true;
            }
            ui.label(RichText::new(&self.audio.input_status).small().color(DIM));
            ui.label("Reference A4");
            ui.add(egui::DragValue::new(&mut self.a4).range(430.0..=450.0).speed(0.1).suffix(" Hz"));
            ui.separator();
            let current = tabs::TUNINGS.iter().find(|(_, t)| *t == self.song.tuning).map(|(n, _)| *n).unwrap_or("Custom");
            egui::ComboBox::from_id_salt("tuning").selected_text(current).show_ui(ui, |ui| {
                for (name, t) in tabs::TUNINGS.iter() {
                    if ui.selectable_label(self.song.tuning == *t, *name).clicked() {
                        self.song.tuning = *t;
                        self.song_dirty = true;
                    }
                }
            });
            ui.checkbox(&mut self.flats, "Flat names");
        });
        if turn_on {
            self.audio.start_input();
        }
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().min(760.0), 200.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 10.0, Color32::from_rgb(20, 22, 26));
        let center = pos2(rect.center().x, rect.bottom() - 20.0);
        let r = 150.0;
        for i in (-50..=50).step_by(5) {
            let a = std::f32::consts::PI * (1.5 + i as f32 / 50.0 * 0.35);
            let len = if i % 25 == 0 { 18.0 } else { 9.0 };
            let col = if i == 0 { Color32::from_rgb(90, 220, 110) } else { Color32::from_gray(120) };
            p.line_segment([center + vec2(a.cos(), a.sin()) * r, center + vec2(a.cos(), a.sin()) * (r - len)], Stroke::new(2.0, col));
        }
        if let Some((midi, cents, f)) = self.tuner {
            self.tuner_smooth += (cents - self.tuner_smooth) * 0.25;
            let n = dsp::NEEDLE_CENTS;
            let a = std::f32::consts::PI * (1.5 + self.tuner_smooth.clamp(-n, n) / n * 0.35);
            let good = cents.round().abs() <= dsp::IN_TUNE_CENTS;
            let col = if good { Color32::from_rgb(90, 230, 110) } else { Color32::from_rgb(250, 160, 50) };
            p.line_segment([center, center + vec2(a.cos(), a.sin()) * (r - 4.0)], Stroke::new(3.0, col));
            p.text(pos2(rect.left() + 90.0, rect.center().y), Align2::CENTER_CENTER, dsp::note_name_in(midi, self.flats), FontId::proportional(48.0), col);
            p.text(pos2(rect.right() - 90.0, rect.center().y - 14.0), Align2::CENTER_CENTER, format!("{:+.0} cents", cents), FontId::monospace(18.0), TEXT);
            p.text(pos2(rect.right() - 90.0, rect.center().y + 14.0), Align2::CENTER_CENTER, format!("{:.1} Hz", f), FontId::monospace(14.0), DIM);
        } else {
            p.text(pos2(rect.left() + 20.0, rect.top() + 24.0), Align2::LEFT_CENTER, "Play a single open string…", FontId::proportional(18.0), DIM);
        }
        p.circle_filled(center, 6.0, Color32::from_gray(200));
        ui.horizontal(|ui| {
            for i in 0..6 {
                let midi = self.song.tuning[5 - i];
                let hit = self.tuner.is_some_and(|(m, _, _)| m == midi);
                ui.label(RichText::new(dsp::note_name_in(midi, self.flats)).size(18.0).color(if hit { Color32::from_rgb(90, 230, 110) } else { DIM }));
                ui.add_space(14.0);
            }
        });
    }

    fn update_tuner(&mut self) {
        let (buf, sr) = {
            let e = self.audio.engine.lock().unwrap();
            let b: Vec<f32> = e.tuner_buf.lock().map(|q| q.iter().copied().collect()).unwrap_or_default();
            (b, e.sr)
        };
        if buf.len() >= 2048 {
            let w = &buf[buf.len() - 2048..];
            self.tuner = dsp::detect_pitch(w, sr).map(|(f, _)| {
                let (m, c) = dsp::freq_to_note(f, self.a4);
                (m, c, f)
            });
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            let path = f.path().to_path_buf();
            if path.as_os_str().is_empty() {
                continue;
            }
            let s = path.to_string_lossy().to_string();
            let lower = s.to_lowercase();
            if lower.ends_with(".txt") || lower.ends_with(".tab") {
                if let Ok(t) = std::fs::read_to_string(&path) {
                    let mut song = tabs::parse_ascii(&t);
                    song.title = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                    self.song = song;
                    self.song_dirty = true;
                    self.page = Page::Tab;
                }
            } else if lower.ends_with(".wav") {
                let sr = self.sr();
                match audio::load_wav(&s, sr) {
                    Ok(x) => {
                        let name = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                        if let Ok(mut e) = self.audio.engine.lock() {
                            e.tracks.push(Track::new(&name, x, [120, 170, 240]));
                        }
                    }
                    Err(e) => self.status = format!("Could not read {s}: {e}"),
                }
                self.local_file = s;
            } else {
                self.local_file = s;
                self.page = Page::Stems;
            }
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        if self.offscreen {
            return;
        }
        let Some(path) = self.shot.clone() else { return };
        self.frames += 1;
        ctx.request_repaint();
        if self.frames >= 120 && !self.shot_requested {
            self.shot_requested = true;
            self.requested_at = Some(Instant::now());
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let img = ctx.input(|i| {
            i.raw.events.iter().chain(i.events.iter()).find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = img {
            if let Err(e) = save_png(&path, &img) {
                eprintln!("screenshot failed: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        // macOS doesn't draw a window it considers hidden (locked screen, covered):
        // draw the same UI off-screen instead.
        let waited = self.requested_at.map_or(0.0, |t| t.elapsed().as_secs_f32());
        if waited > 3.0 || self.started.elapsed().as_secs() > 40 {
            self.requested_at = None;
            self.started = Instant::now() + std::time::Duration::from_secs(3600);
            let ppp = ctx.pixels_per_point().max(1.0);
            let pts = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size())).unwrap_or(egui::vec2(1360.0, 880.0));
            let pts = if pts.x < 400.0 { egui::vec2(1360.0, 880.0) } else { pts };
            let size = [(pts.x * ppp) as u32, (pts.y * ppp) as u32];
            let octx = egui::Context::default();
            let mut app = BandApp::build(&octx, self.shot.clone(), true);
            match crate::offscreen::render(size, ppp, 6, |ui| app.draw(ui), &octx) {
                Ok(img) => {
                    if let Err(e) = save_png(&path, &img) {
                        eprintln!("screenshot failed: {e}");
                    }
                }
                Err(e) => eprintln!("off-screen screenshot failed: {e}"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn save_png(path: &str, img: &egui::ColorImage) -> Result<(), String> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.size[0] as u32, img.size[1] as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    let data: Vec<u8> = img
        .pixels
        .iter()
        .flat_map(|p| {
            let [r, g, b, _] = p.to_array();
            [r, g, b, 255]
        })
        .collect();
    w.write_image_data(&data).map_err(|e| e.to_string())
}

impl eframe::App for BandApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl BandApp {
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.handle_screenshot(&ctx);
        self.handle_drops(&ctx);
        self.poll_jobs();
        if self.page == Page::Tuner || self.audio.input_on() {
            self.update_tuner();
        }
        if self.song_dirty {
            self.push_song();
        }
        let typing = ctx.memory(|m| m.focused().is_some());
        if !typing && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.toggle_play();
        }
        if !typing && self.page != Page::Tab && ctx.input(|i| i.key_pressed(egui::Key::R)) {
            self.record();
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(33));

        egui::Panel::top("controlbar")
            .exact_size(58.0)
            .frame(egui::Frame::new().fill(BAR).inner_margin(egui::Margin::symmetric(10, 6)))
            .show(ui, |ui| self.control_bar(ui));
        egui::Panel::bottom("status").exact_size(22.0).frame(egui::Frame::new().fill(Color32::from_rgb(36, 36, 38)).inner_margin(egui::Margin::symmetric(10, 3))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.audio.status).small().color(DIM));
                ui.label(RichText::new("·").small().color(DIM));
                ui.label(RichText::new(&self.audio.input_status).small().color(DIM));
                ui.label(RichText::new("·").small().color(DIM));
                ui.label(RichText::new(&self.status).small().color(TEXT));
            });
        });
        egui::Panel::bottom("editor").default_size(380.0).min_size(240.0).resizable(true).frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::same(10))).show(ui, |ui| {
            ui.horizontal(|ui| {
                for (pg, label) in [(Page::Amp, "Amp & Cabinet"), (Page::Pedals, "Pedalboard"), (Page::Tab, "♫ Guitar Tab"), (Page::Stems, "Stem Splitter"), (Page::Ai, "AI Assistant"), (Page::Tuner, "♪ Tuner")] {
                    let sel = self.page == pg;
                    let b = egui::Button::new(RichText::new(label).size(13.0).color(if sel { Color32::BLACK } else { TEXT })).fill(if sel { ACCENT } else { BAR }).min_size(vec2(0.0, 26.0));
                    if ui.add(b).clicked() {
                        self.page = pg;
                    }
                }
            });
            ui.separator();
            match self.page {
                Page::Amp => self.amp_page(ui),
                Page::Pedals => self.pedals_page(ui),
                Page::Tab => self.tab_page(ui),
                Page::Stems => self.stems_page(ui),
                Page::Ai => self.ai_page(ui),
                Page::Tuner => self.tuner_page(ui),
            }
        });
        if self.show_library {
            egui::Panel::left("library").exact_size(200.0).frame(egui::Frame::new().fill(Color32::from_rgb(38, 38, 41)).inner_margin(egui::Margin::same(8))).show(ui, |ui| self.library(ui));
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG)).show(ui, |ui| self.tracks(ui));
    }
}
