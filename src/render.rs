//! Key images (144×144 SVG) and dial feedback for the Stream Deck+ strip.
//!
//! Icons and colours are WaveSink's own, so a key looks like the strip it
//! controls.

use base64::Engine;
use serde_json::{Value, json};

const FONT: &str = "Liberation Sans, Arial, sans-serif";
const BG: &str = "#0f0f12";
const MUTED_RED: &str = "#ef4444";
const ON_GREEN: &str = "#22c55e";
const DIM: &str = "#71717a";

/// WaveSink's channel icons: 24×24 stroke paths (`ChannelIcon.tsx`).
pub fn icon_path(id: Option<&str>) -> &'static str {
	let id = match id.unwrap_or("") {
		"sports_esports" => "game",
		"music_note" => "music",
		"desktop_windows" => "system",
		"globe" => "public",
		other => other,
	};
	match id {
		"game" => {
			"M7 8h10l2 4v5a2 2 0 0 1-2 2l-3-2H10l-3 2a2 2 0 0 1-2-2v-5l2-4Zm2.5 4.5v3m-1.5-1.5h3m5.5-1.5h.01m-2 2h.01"
		}
		"chat" => "M5 5h14v10H9l-4 4V5Zm4 5h.01m3 0h.01m3 0h.01",
		"music" => {
			"M9 18V6l10-2v12M9 18a2 2 0 1 1-2-2 2 2 0 0 1 2 2Zm10-2a2 2 0 1 1-2-2 2 2 0 0 1 2 2Z"
		}
		"system" => "M4 5h16v11H4V5Zm6 14h4M8 20h8",
		"mic" => {
			"M12 14a3 3 0 0 0 3-3V6a3 3 0 0 0-6 0v5a3 3 0 0 0 3 3Zm-5-3a5 5 0 0 0 10 0m-5 5v4m-3 0h6"
		}
		"broadcast" => "M5 15V9l10-4v14L5 15Zm12-5a3 3 0 0 1 0 4m2-6a6 6 0 0 1 0 8",
		"browser" => "M4 5h16v14H4V5Zm0 4h16M7 7h.01m3 0h.01m3 0h.01",
		"terminal" => "M5 5h14v14H5V5Zm3 4 3 3-3 3m5 0h3",
		"star" => "m12 4 2.4 4.9 5.4.8-3.9 3.8.9 5.4-4.8-2.5-4.8 2.5.9-5.4-3.9-3.8 5.4-.8L12 4Z",
		"headphones" => "M4 14v-2a8 8 0 0 1 16 0v2m-16 0v4h4v-5H4m16 1v4h-4v-5h4",
		"public" => {
			"M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16Zm-8 8h16M12 4c2.5 2.2 3.5 5 3.5 8s-1 5.8-3.5 8c-2.5-2.2-3.5-5-3.5-8s1-5.8 3.5-8"
		}
		"webcam" => "M6 7h12v10H6V7Zm3 13h6m-3-3v3m-1.5-9.5a1.5 1.5 0 1 0 3 0 1.5 1.5 0 0 0-3 0",
		"camera" => "M4 8h4l1.5-2h5L16 8h4v10H4V8Zm8 2a3 3 0 1 0 0 6 3 3 0 0 0 0-6",
		"forum" => "M4 5h12v9H9l-4 4V5Zm6 11h6l4 3v-9h-2",
		_ => "M12 4v16M4 12h16",
	}
}

pub const PROFILE_ICON: &str = "m12 4 8 4-8 4-8-4 8-4Zm-8 8 8 4 8-4m-16 4 8 4 8-4";
pub const WAVE_ICON: &str = "M3 12h2l2-6 3 12 3-9 2 5 2-2h4";
const MUTE_GLYPH: &str = "M4 9h4l5-4v14l-5-4H4V9Zm12 0 5 6m0-6-5 6";

/// WaveSink's icon colour tokens (`globals.css`).
pub fn color_hex(token: Option<&str>, fallback: &'static str) -> &'static str {
	match token.unwrap_or(fallback) {
		"slate" => "#475569",
		"gray" => "#6b7280",
		"red" => "#dc2626",
		"orange" => "#ea580c",
		"amber" => "#d97706",
		"yellow" => "#ca8a04",
		"lime" => "#65a30d",
		"green" => "#16a34a",
		"teal" => "#0f766e",
		"cyan" => "#0891b2",
		"blue" => "#2563eb",
		"purple" => "#7c3aed",
		_ if token.is_some() => color_hex(None, fallback),
		_ => "#2563eb",
	}
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
	pub path: &'static str,
	pub color: &'static str,
}

fn escape(text: &str) -> String {
	text.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
}

/// Keys are small; long names get an ellipsis rather than overflowing.
pub fn fit(text: &str, max: usize) -> String {
	if text.chars().count() <= max {
		text.to_owned()
	} else {
		let cut: String = text.chars().take(max.saturating_sub(1)).collect();
		format!("{}…", cut.trim_end())
	}
}

pub fn data_url(svg: &str) -> String {
	format!(
		"data:image/svg+xml;base64,{}",
		base64::engine::general_purpose::STANDARD.encode(svg)
	)
}

fn tile_svg(tile: Tile, x: f32, y: f32, size: f32, glyph: Option<&str>) -> String {
	let scale = size * 0.6 / 24.0;
	let inset = size * 0.2;
	format!(
		r##"<rect x="{x}" y="{y}" width="{size}" height="{size}" rx="{r}" fill="{color}"/><g transform="translate({gx} {gy}) scale({scale})" fill="none" stroke="#fff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="{path}"/></g>"##,
		r = size * 0.22,
		color = tile.color,
		gx = x + inset,
		gy = y + inset,
		path = glyph.unwrap_or(tile.path),
	)
}

fn key(body: String, border: Option<&str>) -> String {
	let frame = border
		.map(|c| format!(r#"<rect x="3" y="3" width="138" height="138" rx="14" fill="none" stroke="{c}" stroke-width="6"/>"#))
		.unwrap_or_default();
	format!(
		r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 144 144" width="144" height="144"><rect width="144" height="144" fill="{BG}"/>{body}{frame}</svg>"#
	)
}

fn text(x: f32, y: f32, size: u32, weight: u32, fill: &str, anchor: &str, value: &str) -> String {
	format!(
		r#"<text x="{x}" y="{y}" font-family="{FONT}" font-size="{size}" font-weight="{weight}" fill="{fill}" text-anchor="{anchor}">{}</text>"#,
		escape(value)
	)
}

pub struct LevelKey<'a> {
	pub tile: Tile,
	pub label: &'a str,
	pub caption: &'a str,
	pub volume: u8,
	pub muted: bool,
	/// False when the level can't be heard (an unrouted cell, a muted
	/// input under a send): drawn dimmed.
	pub live: bool,
}

/// An input, a send or a mix: icon, level, name and a level bar.
pub fn level_key(k: &LevelKey) -> String {
	let tile = if k.muted || !k.live {
		Tile {
			path: k.tile.path,
			color: color_hex(Some("gray"), "gray"),
		}
	} else {
		k.tile
	};
	let glyph = k.muted.then_some(MUTE_GLYPH);
	let (value, value_color) = if k.muted {
		("MUTE".to_owned(), MUTED_RED)
	} else {
		(
			format!("{}%", k.volume),
			if k.live { "#fafafa" } else { DIM },
		)
	};
	let bar_fill = if k.muted || !k.live {
		DIM
	} else {
		k.tile.color
	};
	let width = 120.0 * f32::from(k.volume.min(100)) / 100.0;
	let body = [
		tile_svg(tile, 12.0, 12.0, 52.0, glyph),
		text(134.0, 48.0, if k.muted { 20 } else { 28 }, 700, value_color, "end", &value),
		text(72.0, 92.0, 21, 700, if k.live { "#fafafa" } else { DIM }, "middle", &fit(k.label, 11)),
		text(72.0, 113.0, 15, 400, "#a1a1aa", "middle", &fit(k.caption, 15)),
		format!(r##"<rect x="12" y="122" width="120" height="10" rx="5" fill="#3f3f46"/><rect x="12" y="122" width="{width}" height="10" rx="5" fill="{bar_fill}"/>"##),
	]
	.concat();
	key(body, k.muted.then_some(MUTED_RED))
}

pub struct ToggleKey<'a> {
	pub tile: Tile,
	pub label: &'a str,
	pub caption: &'a str,
	pub badge: &'a str,
	pub on: bool,
}

/// State words sit right of the icon tile: longer ones get smaller.
fn badge_size(badge: &str) -> u32 {
	match badge.chars().count() {
		0..=2 => 28,
		3 => 26,
		_ => 20,
	}
}

/// A route, a solo or a profile: big state word, green frame while on.
pub fn toggle_key(k: &ToggleKey) -> String {
	let tile = if k.on {
		k.tile
	} else {
		Tile {
			path: k.tile.path,
			color: "#3f3f46",
		}
	};
	let body = [
		tile_svg(tile, 12.0, 12.0, 52.0, None),
		text(
			134.0,
			48.0,
			badge_size(k.badge),
			800,
			if k.on { ON_GREEN } else { DIM },
			"end",
			k.badge,
		),
		text(72.0, 96.0, 21, 700, "#fafafa", "middle", &fit(k.label, 11)),
		text(
			72.0,
			122.0,
			15,
			400,
			"#a1a1aa",
			"middle",
			&fit(k.caption, 16),
		),
	]
	.concat();
	key(body, k.on.then_some(ON_GREEN))
}

/// Offline, or not set up yet.
pub fn notice_key(title: &str, line: &str) -> String {
	let tile = Tile {
		path: WAVE_ICON,
		color: "#27272a",
	};
	let body = [
		tile_svg(tile, 44.0, 14.0, 56.0, None),
		text(72.0, 100.0, 20, 700, "#d4d4d8", "middle", title),
		text(72.0, 122.0, 15, 400, DIM, "middle", line),
	]
	.concat();
	key(body, None)
}

/// The icon pixmap on the dial strip, as a bare SVG string.
fn dial_icon(tile: Tile, glyph: Option<&str>) -> String {
	format!(
		r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40" width="40" height="40">{}</svg>"#,
		tile_svg(tile, 0.0, 0.0, 40.0, glyph)
	)
}

/// Linear peak to a 0-100 bar over -60..0 dBFS, the range the ear uses.
pub fn meter_value(peak: f32) -> u8 {
	if peak <= 0.001 {
		return 0;
	}
	let db = 20.0 * peak.log10();
	(((db + 60.0) / 60.0).clamp(0.0, 1.0) * 100.0).round() as u8
}

pub struct Dial<'a> {
	pub tile: Tile,
	pub title: &'a str,
	pub volume: Option<u8>,
	pub value: Option<String>,
	pub muted: bool,
	pub live: bool,
	pub meter: Option<u8>,
}

pub fn dial(d: &Dial) -> Value {
	let tile = if d.muted || !d.live {
		Tile {
			path: d.tile.path,
			color: color_hex(Some("gray"), "gray"),
		}
	} else {
		d.tile
	};
	let (value, color) = match (&d.value, d.muted) {
		(_, true) => ("Muted".to_owned(), MUTED_RED),
		(Some(v), false) => (v.clone(), if d.live { "white" } else { DIM }),
		(None, false) => (
			d.volume.map(|v| format!("{v}%")).unwrap_or_default(),
			if d.live { "white" } else { DIM },
		),
	};
	let fill = if d.muted || !d.live {
		DIM
	} else {
		d.tile.color
	};
	json!({
		"icon": dial_icon(tile, d.muted.then_some(MUTE_GLYPH)),
		"title": d.title,
		"value": { "value": value, "color": color },
		"volume": { "value": d.volume.unwrap_or(0), "bar_fill_c": fill, "enabled": d.volume.is_some() },
		"meter": { "value": d.meter.unwrap_or(0), "enabled": d.meter.is_some() },
	})
}

pub fn dial_notice(title: &str, line: &str) -> Value {
	json!({
		"icon": dial_icon(Tile { path: WAVE_ICON, color: "#27272a" }, None),
		"title": title,
		"value": { "value": line, "color": DIM },
		"volume": { "value": 0, "enabled": false },
		"meter": { "value": 0, "enabled": false },
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn meter_maps_dbfs_onto_the_bar() {
		assert_eq!(meter_value(0.0), 0);
		assert_eq!(meter_value(1.0), 100);
		assert_eq!(meter_value(0.001), 0);
		// -20 dBFS is two thirds of the way up.
		assert_eq!(meter_value(0.1), 67);
	}

	#[test]
	fn labels_are_fitted_and_escaped() {
		assert_eq!(fit("Game", 11), "Game");
		assert_eq!(fit("Very Long Channel Name", 8), "Very Lo…");
		let svg = level_key(&LevelKey {
			tile: Tile {
				path: icon_path(Some("game")),
				color: color_hex(None, "blue"),
			},
			label: "R&B <live>",
			caption: "All mixes",
			volume: 40,
			muted: false,
			live: true,
		});
		assert!(svg.contains("R&amp;B &lt;live&gt;"));
		assert!(svg.contains("40%"));
	}

	#[test]
	fn unknown_tokens_fall_back() {
		assert_eq!(color_hex(Some("chartreuse"), "purple"), "#7c3aed");
		assert_eq!(color_hex(None, "blue"), "#2563eb");
		assert_eq!(icon_path(Some("sports_esports")), icon_path(Some("game")));
		assert_eq!(icon_path(None), icon_path(Some("generic")));
	}

	#[test]
	fn data_urls_round_trip() {
		let url = data_url("<svg/>");
		let payload = url
			.strip_prefix("data:image/svg+xml;base64,")
			.expect("prefix");
		let bytes = base64::engine::general_purpose::STANDARD
			.decode(payload)
			.expect("base64");
		assert_eq!(bytes, b"<svg/>");
	}
}
