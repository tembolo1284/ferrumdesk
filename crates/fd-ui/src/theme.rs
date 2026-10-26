use egui::{Color32, Visuals};

pub const UP: Color32 = Color32::from_rgb(0x3f, 0xb9, 0x50);
pub const DOWN: Color32 = Color32::from_rgb(0xe5, 0x53, 0x4b);
pub const MUTED: Color32 = Color32::from_rgb(0x8a, 0x8f, 0x98);
pub const ACCENT: Color32 = Color32::from_rgb(0x5b, 0x9c, 0xf5);

pub fn apply(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = Color32::from_rgb(0x14, 0x16, 0x1a);
    v.window_fill = Color32::from_rgb(0x1b, 0x1e, 0x24);
    v.faint_bg_color = Color32::from_rgb(0x1f, 0x22, 0x28);
    v.extreme_bg_color = Color32::from_rgb(0x0f, 0x10, 0x13);
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 3.0);
    use egui::{FontFamily, FontId, TextStyle};
    style.text_styles.insert(TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace));
    style.text_styles.insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
    ctx.set_style(style);
}

pub fn pnl_color(x: f64) -> Color32 {
    if x > 0.0 { UP } else if x < 0.0 { DOWN } else { MUTED }
}

/// Fixed-width number formatting for grids.
pub fn money(x: f64) -> String {
    let neg = x < 0.0;
    let s = format!("{:.0}", x.abs());
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); }
        out.push(c);
    }
    if neg { format!("-{out}") } else { out }
}

pub fn pct(x: f64) -> String { format!("{:.4}%", x * 100.0) }
pub fn bp(x: f64) -> String { format!("{:.2}", x * 1e4) }
