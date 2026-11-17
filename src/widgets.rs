//! Drawn controls: rotary knobs, stomp boxes, meters.

use egui::{pos2, vec2, Align2, Color32, FontId, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui};
use std::f32::consts::PI;

pub fn c(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// A rotary knob. Drag up/down (or scroll) to turn; double-click resets.
pub fn knob(ui: &mut Ui, value: &mut f32, min: f32, max: f32, default: f32, label: &str, size: f32, cap: Color32, text: Color32) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(size + 14.0, size + 30.0), Sense::click_and_drag());
    let old = *value;
    if resp.dragged() {
        let d = -resp.drag_delta().y + resp.drag_delta().x * 0.5;
        *value = (*value + d / 150.0 * (max - min)).clamp(min, max);
    }
    if resp.hovered() {
        let s = ui.input(|i| i.smooth_scroll_delta.y);
        if s != 0.0 {
            *value = (*value + s / 600.0 * (max - min)).clamp(min, max);
        }
    }
    if resp.double_clicked() {
        *value = default;
    }
    if *value != old {
        resp.mark_changed();
    }
    let p = ui.painter();
    let center = pos2(rect.center().x, rect.top() + 4.0 + size / 2.0);
    let r = size / 2.0;
    let t = (*value - min) / (max - min);
    let a0 = PI * 0.75;
    let sweep = PI * 1.5;
    // track arc
    let arc = |from: f32, to: f32| -> Vec<egui::Pos2> {
        let n = 32;
        (0..=n).map(|i| {
            let a = from + (to - from) * i as f32 / n as f32;
            center + vec2(a.cos(), a.sin()) * (r + 4.0)
        }).collect()
    };
    p.add(Shape::line(arc(a0, a0 + sweep), Stroke::new(3.0, Color32::from_black_alpha(140))));
    p.add(Shape::line(arc(a0, a0 + sweep * t), Stroke::new(3.0, Color32::from_rgb(255, 170, 60))));
    p.circle_filled(center + vec2(0.0, 2.0), r, Color32::from_black_alpha(120));
    p.circle_filled(center, r, cap);
    p.circle_stroke(center, r, Stroke::new(1.0, Color32::from_white_alpha(60)));
    p.circle_filled(center, r * 0.72, cap.gamma_multiply(0.85));
    let a = a0 + sweep * t;
    p.line_segment([center + vec2(a.cos(), a.sin()) * r * 0.25, center + vec2(a.cos(), a.sin()) * r * 0.9], Stroke::new(2.5, Color32::WHITE));
    p.text(pos2(rect.center().x, rect.bottom() - 18.0), Align2::CENTER_CENTER, label, FontId::proportional(11.0), text);
    let shown = if max - min > 20.0 { format!("{:.0}", value) } else if max <= 1.0 { format!("{:.0}%", *value * 100.0) } else { format!("{:.1}", value) };
    p.text(pos2(rect.center().x, rect.bottom() - 6.0), Align2::CENTER_CENTER, shown, FontId::monospace(10.0), text.gamma_multiply(0.7));
    resp
}

/// Result of drawing one stomp box.
#[derive(Default)]
pub struct PedalAction {
    pub changed: bool,
    pub remove: bool,
    pub left: bool,
    pub right: bool,
}

/// Draw a stomp box with three knobs and a footswitch.
pub fn pedal(ui: &mut Ui, slot: &mut crate::dsp::PedalSlot, idx: usize) -> PedalAction {
    let info = crate::dsp::pedal_info(slot.kind);
    let mut act = PedalAction::default();
    let w = 128.0;
    let h = 210.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let body = c(info.color);
    let dark = (body.r() as u32 + body.g() as u32 + body.b() as u32) < 300;
    let ink = if dark { Color32::from_gray(235) } else { Color32::from_gray(25) };
    {
        let p = ui.painter();
        p.rect_filled(rect.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(120));
        p.rect_filled(rect, 10.0, body);
        p.rect_stroke(rect, 10.0, Stroke::new(1.0, Color32::from_white_alpha(50)), StrokeKind::Inside);
        // LED
        let led = pos2(rect.center().x, rect.top() + 12.0);
        p.circle_filled(led, 4.0, if slot.on { Color32::from_rgb(255, 40, 40) } else { Color32::from_rgb(70, 20, 20) });
        if slot.on {
            p.circle_filled(led, 8.0, Color32::from_rgba_unmultiplied(255, 40, 40, 40));
        }
        p.text(pos2(rect.center().x, rect.top() + 120.0), Align2::CENTER_CENTER, info.name, FontId::proportional(14.0), ink);
        p.text(pos2(rect.center().x, rect.top() + 136.0), Align2::CENTER_CENTER, info.category.to_uppercase(), FontId::proportional(9.0), ink.gamma_multiply(0.6));
    }
    // knobs
    let krect = Rect::from_min_size(rect.min + vec2(2.0, 22.0), vec2(w - 4.0, 80.0));
    let mut kui = ui.new_child(egui::UiBuilder::new().max_rect(krect).layout(egui::Layout::left_to_right(egui::Align::Min)));
    kui.spacing_mut().item_spacing.x = 0.0;
    let cap = if dark { Color32::from_gray(200) } else { Color32::from_gray(30) };
    for k in 0..3 {
        let r = knob(&mut kui, &mut slot.knobs[k], 0.0, 1.0, 0.5, info.knobs[k], 26.0, cap, ink);
        act.changed |= r.changed();
    }
    // footswitch
    let fs_center = pos2(rect.center().x, rect.bottom() - 42.0);
    let fs = Rect::from_center_size(fs_center, vec2(36.0, 36.0));
    let r = ui.interact(fs, egui::Id::new(("fs", idx)), Sense::click());
    {
        let p = ui.painter();
        p.circle_filled(fs_center, 17.0, Color32::from_gray(150));
        p.circle_filled(fs_center, 13.0, if r.hovered() { Color32::from_gray(225) } else { Color32::from_gray(195) });
        p.circle_stroke(fs_center, 17.0, Stroke::new(1.0, Color32::from_black_alpha(140)));
    }
    if r.on_hover_text("Click to switch on/off").clicked() {
        slot.on = !slot.on;
        act.changed = true;
    }
    // small buttons
    let row = Rect::from_min_size(pos2(rect.left() + 6.0, rect.bottom() - 18.0), vec2(w - 12.0, 16.0));
    let btn = |ui: &mut Ui, r: Rect, id: &str, t: &str| {
        let resp = ui.interact(r, egui::Id::new((id, idx)), Sense::click());
        ui.painter().text(r.center(), Align2::CENTER_CENTER, t, FontId::proportional(11.0), if resp.hovered() { ink } else { ink.gamma_multiply(0.55) });
        resp.clicked()
    };
    act.left = btn(ui, Rect::from_min_size(row.min, vec2(20.0, 16.0)), "l", "◀");
    act.remove = btn(ui, Rect::from_center_size(row.center(), vec2(20.0, 16.0)), "x", "×");
    act.right = btn(ui, Rect::from_min_size(pos2(row.right() - 20.0, row.top()), vec2(20.0, 16.0)), "r", "▶");
    act
}

/// Horizontal level meter.
pub fn meter(ui: &mut Ui, level: f32, w: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(w, 8.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, Color32::from_gray(25));
    let l = (level * 6.0).clamp(0.0, 1.0);
    let fill = Rect::from_min_size(rect.min, vec2(rect.width() * l, rect.height()));
    let col = if l > 0.9 { Color32::from_rgb(230, 60, 50) } else if l > 0.7 { Color32::from_rgb(230, 200, 60) } else { Color32::from_rgb(80, 200, 90) };
    p.rect_filled(fill, 2.0, col);
}
