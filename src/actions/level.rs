//! Input and Mix: a level and its mute.
//!
//! An Input targets the input's own fader, which feeds every mix, or one of
//! its sends (the cell in a mix). On a dial: rotate for level, push to mute,
//! tap to reset the level (or optionally mute). On a key: mute, step up or
//! down, or jump to a level.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{Settings, is_dial, paint_dial, paint_key, paint_offline, paint_unset, sent};
use crate::client;
use crate::model::Snapshot;
use crate::render::{self, Dial, LevelKey, Tile};

pub const INPUT: &str = "us.echo.wavesink.input";
pub const MIX: &str = "us.echo.wavesink.mix";

const DEFAULT_STEP: u8 = 5;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct LevelSettings {
	/// Input id (Input) or mix id (Mix).
	pub target: String,
	/// Input only: a mix id to control that send, or empty for the fader.
	pub mix: String,
	/// Key press: "mute" (default), "up", "down" or "set".
	pub press: String,
	/// Percent per dial tick or key press.
	pub step: Option<u8>,
	/// The level "set" jumps to.
	pub value: Option<u8>,
	/// Touchscreen tap: "reset" (default) or "mute".
	pub touch: Option<String>,
	/// The level a touchscreen reset returns to (default 100%).
	pub default_level: Option<u8>,
	/// Show the live meter on the dial (default on).
	pub meter: Option<bool>,
}

impl LevelSettings {
	fn step(&self) -> i16 {
		i16::from(self.step.unwrap_or(DEFAULT_STEP).clamp(1, 50))
	}

	fn meter(&self) -> bool {
		self.meter.unwrap_or(true)
	}

	fn touch_volume(&self) -> Option<u8> {
		(self.touch.as_deref() != Some("mute")).then(|| self.default_level.unwrap_or(100).min(100))
	}
}

static INPUTS: Settings<LevelSettings> = Settings::new();
static MIXES: Settings<LevelSettings> = Settings::new();

fn store(uuid: &str) -> &'static Settings<LevelSettings> {
	if uuid == MIX { &MIXES } else { &INPUTS }
}

/// Dials showing a meter. Keys never draw one, whatever their settings say.
static METER_DIALS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

fn track_meter(instance: &Instance, settings: Option<&LevelSettings>) {
	let mut dials = METER_DIALS.lock().unwrap_or_else(|e| e.into_inner());
	if is_dial(instance) && settings.is_some_and(LevelSettings::meter) {
		dials.insert(instance.instance_id.clone());
	} else {
		dials.remove(&instance.instance_id);
	}
}

pub fn wants_meters() -> bool {
	!METER_DIALS
		.lock()
		.unwrap_or_else(|e| e.into_inner())
		.is_empty()
}

/// What a level control shows, resolved against the live snapshot.
#[derive(Debug, PartialEq)]
struct View {
	tile: Tile,
	label: String,
	caption: String,
	volume: u8,
	muted: bool,
	live: bool,
	/// (is a mix, id) for the meter.
	meter: (bool, String),
	/// A send hears its input scaled by the send level.
	meter_scale: f32,
	/// The dial strip's title. Its font has no arrows, so a send reads
	/// "Mic » Stream" there.
	title: String,
}

fn view(uuid: &str, settings: &LevelSettings, snapshot: &Snapshot) -> Option<View> {
	if uuid == MIX {
		let mix = snapshot.mix(&settings.target)?;
		return Some(View {
			tile: Tile {
				path: render::icon_path(mix.icon.as_deref()),
				color: render::color_hex(mix.icon_color.as_deref(), "purple"),
			},
			label: mix.label.clone(),
			caption: "Mix".into(),
			volume: mix.volume,
			muted: mix.muted,
			live: true,
			meter: (true, mix.id.clone()),
			meter_scale: 1.0,
			title: mix.label.clone(),
		});
	}
	let input = snapshot.input(&settings.target)?;
	let tile = Tile {
		path: render::icon_path(input.icon.as_deref()),
		color: render::color_hex(input.icon_color.as_deref(), "blue"),
	};
	if settings.mix.is_empty() {
		return Some(View {
			tile,
			label: input.label.clone(),
			caption: "All mixes".into(),
			volume: input.volume,
			muted: input.muted,
			live: true,
			meter: (false, input.id.clone()),
			meter_scale: 1.0,
			title: input.label.clone(),
		});
	}
	let mix = snapshot.mix(&settings.mix)?;
	let cell = input.routes.get(&mix.id).cloned().unwrap_or_default();
	let caption = if !cell.enabled {
		format!("{} · off", mix.label)
	} else if input.muted {
		"input muted".into()
	} else {
		format!("→ {}", mix.label)
	};
	Some(View {
		tile,
		label: input.label.clone(),
		caption,
		volume: cell.send,
		muted: cell.muted,
		live: cell.enabled && !input.muted,
		meter: (false, input.id.clone()),
		meter_scale: f32::from(cell.send) / 100.0,
		title: format!("{} » {}", input.label, mix.label),
	})
}

fn meter_for(view: &View, settings: &LevelSettings) -> Option<u8> {
	if !settings.meter() {
		return None;
	}
	if view.muted || !view.live {
		return Some(0);
	}
	let (mix, id) = &view.meter;
	let peak = client::with_levels(|l| {
		if *mix {
			l.mixes.get(id)
		} else {
			l.inputs.get(id)
		}
		.copied()
		.unwrap_or(0.0)
	});
	Some(render::meter_value(peak * view.meter_scale))
}

/// Give a freshly placed action the first input or mix, so it does
/// something before anyone opens the inspector.
async fn default_target(instance: &Instance, settings: &mut LevelSettings) -> OpenActionResult<()> {
	if !settings.target.is_empty() {
		return Ok(());
	}
	let first = client::with_snapshot(|s| {
		s.and_then(|s| {
			if instance.action_uuid == MIX {
				s.mixes.first().map(|m| m.id.clone())
			} else {
				s.inputs.first().map(|i| i.id.clone())
			}
		})
	});
	if let Some(id) = first {
		settings.target = id;
		store(&instance.action_uuid).put(&instance.instance_id, settings.clone());
		instance.set_settings(settings).await?;
	}
	Ok(())
}

pub async fn paint(instance: &Instance) -> OpenActionResult<()> {
	let Some(mut settings) = store(&instance.action_uuid).get(&instance.instance_id) else {
		return Ok(());
	};
	if !client::connected() || client::with_snapshot(|s| s.is_none()) {
		return paint_offline(instance).await;
	}
	default_target(instance, &mut settings).await?;
	let Some(view) =
		client::with_snapshot(|s| s.and_then(|s| view(&instance.action_uuid, &settings, s)))
	else {
		let what = if instance.action_uuid == MIX {
			"a mix"
		} else {
			"an input"
		};
		return paint_unset(instance, what).await;
	};
	if is_dial(instance) {
		paint_dial(
			instance,
			render::dial(&Dial {
				tile: view.tile,
				title: &view.title,
				volume: Some(view.volume),
				value: None,
				muted: view.muted,
				live: view.live,
				meter: meter_for(&view, &settings),
			}),
		)
		.await
	} else {
		paint_key(
			instance,
			render::level_key(&LevelKey {
				tile: view.tile,
				label: &view.label,
				caption: &view.caption,
				volume: view.volume,
				muted: view.muted,
				live: view.live,
			}),
		)
		.await
	}
}

pub async fn paint_meter(instance: &Instance) -> OpenActionResult<()> {
	if !is_dial(instance) {
		return Ok(());
	}
	let Some(settings) = store(&instance.action_uuid).get(&instance.instance_id) else {
		return Ok(());
	};
	let value =
		client::with_snapshot(|s| s.and_then(|s| view(&instance.action_uuid, &settings, s)))
			.and_then(|view| meter_for(&view, &settings));
	match value {
		Some(value) => super::paint_meter(instance, value).await,
		None => Ok(()),
	}
}

fn adjust(uuid: &str, settings: &LevelSettings, delta: i16) -> bool {
	if uuid == MIX {
		client::call(
			"adjust_mix_volume",
			json!({ "mix": settings.target, "delta": delta }),
		)
	} else if settings.mix.is_empty() {
		client::call(
			"adjust_input_volume",
			json!({ "input": settings.target, "delta": delta }),
		)
	} else {
		client::call(
			"adjust_route_send",
			json!({ "input": settings.target, "mix": settings.mix, "delta": delta }),
		)
	}
}

fn set(uuid: &str, settings: &LevelSettings, volume: u8) -> bool {
	if uuid == MIX {
		client::call(
			"set_mix_volume",
			json!({ "mix": settings.target, "volume": volume }),
		)
	} else if settings.mix.is_empty() {
		client::call(
			"set_input_volume",
			json!({ "input": settings.target, "volume": volume }),
		)
	} else {
		client::call(
			"set_route",
			json!({ "input": settings.target, "mix": settings.mix, "send": volume }),
		)
	}
}

fn toggle_mute(uuid: &str, settings: &LevelSettings) -> bool {
	if uuid == MIX {
		client::call("set_mix_mute", json!({ "mix": settings.target }))
	} else if settings.mix.is_empty() {
		client::call("set_input_mute", json!({ "input": settings.target }))
	} else {
		client::call(
			"toggle_route",
			json!({ "input": settings.target, "mix": settings.mix, "field": "muted" }),
		)
	}
}

fn known(uuid: &str, settings: &LevelSettings) -> bool {
	client::with_snapshot(|s| s.and_then(|s| view(uuid, settings, s)).is_some())
}

async fn appear(instance: &Instance, settings: &LevelSettings) -> OpenActionResult<()> {
	super::forget(&instance.instance_id);
	store(&instance.action_uuid).put(&instance.instance_id, settings.clone());
	track_meter(instance, Some(settings));
	super::update_meter_interest();
	paint(instance).await
}

async fn disappear(instance: &Instance) {
	store(&instance.action_uuid).remove(&instance.instance_id);
	track_meter(instance, None);
	super::forget(&instance.instance_id);
	super::inspector_closed(instance);
	super::update_meter_interest();
}

async fn press(instance: &Instance, settings: &LevelSettings) -> OpenActionResult<()> {
	let uuid = &instance.action_uuid;
	if !known(uuid, settings) {
		return sent(instance, false).await;
	}
	let ok = match settings.press.as_str() {
		"up" => adjust(uuid, settings, settings.step()),
		"down" => adjust(uuid, settings, -settings.step()),
		"set" => set(uuid, settings, settings.value.unwrap_or(100).min(100)),
		_ => toggle_mute(uuid, settings),
	};
	sent(instance, ok).await
}

async fn rotate(instance: &Instance, settings: &LevelSettings, ticks: i16) -> OpenActionResult<()> {
	let uuid = &instance.action_uuid;
	if !known(uuid, settings) {
		return sent(instance, false).await;
	}
	sent(
		instance,
		adjust(uuid, settings, ticks.saturating_mul(settings.step())),
	)
	.await
}

async fn mute(instance: &Instance, settings: &LevelSettings) -> OpenActionResult<()> {
	let uuid = &instance.action_uuid;
	let ok = known(uuid, settings) && toggle_mute(uuid, settings);
	sent(instance, ok).await
}

async fn touch(instance: &Instance, settings: &LevelSettings) -> OpenActionResult<()> {
	let uuid = &instance.action_uuid;
	if !known(uuid, settings) {
		return sent(instance, false).await;
	}
	let ok = match settings.touch_volume() {
		Some(volume) => set(uuid, settings, volume),
		None => toggle_mute(uuid, settings),
	};
	sent(instance, ok).await
}

macro_rules! level_action {
	($name:ident, $uuid:expr) => {
		pub struct $name;

		#[async_trait]
		impl Action for $name {
			const UUID: &'static str = $uuid;
			type Settings = LevelSettings;

			async fn will_appear(
				&self,
				instance: &Instance,
				settings: &Self::Settings,
			) -> OpenActionResult<()> {
				appear(instance, settings).await
			}

			async fn will_disappear(
				&self,
				instance: &Instance,
				_settings: &Self::Settings,
			) -> OpenActionResult<()> {
				disappear(instance).await;
				Ok(())
			}

			async fn did_receive_settings(
				&self,
				instance: &Instance,
				settings: &Self::Settings,
			) -> OpenActionResult<()> {
				store(&instance.action_uuid).put(&instance.instance_id, settings.clone());
				track_meter(instance, Some(settings));
				super::update_meter_interest();
				paint(instance).await
			}

			async fn key_down(
				&self,
				instance: &Instance,
				settings: &Self::Settings,
			) -> OpenActionResult<()> {
				press(instance, settings).await
			}

			async fn dial_rotate(
				&self,
				instance: &Instance,
				settings: &Self::Settings,
				ticks: i16,
				_pressed: bool,
			) -> OpenActionResult<()> {
				rotate(instance, settings, ticks).await
			}

			async fn dial_down(
				&self,
				instance: &Instance,
				settings: &Self::Settings,
			) -> OpenActionResult<()> {
				mute(instance, settings).await
			}

			async fn touch_tap(
				&self,
				instance: &Instance,
				settings: &Self::Settings,
				_position: (u16, u16),
				_hold: bool,
			) -> OpenActionResult<()> {
				touch(instance, settings).await
			}

			async fn property_inspector_did_appear(
				&self,
				instance: &Instance,
				_settings: &Self::Settings,
			) -> OpenActionResult<()> {
				super::inspector_opened(instance).await
			}

			async fn property_inspector_did_disappear(
				&self,
				instance: &Instance,
				_settings: &Self::Settings,
			) -> OpenActionResult<()> {
				super::inspector_closed(instance);
				Ok(())
			}

			async fn send_to_plugin(
				&self,
				instance: &Instance,
				_settings: &Self::Settings,
				payload: &serde_json::Value,
			) -> OpenActionResult<()> {
				super::inspector_message(instance, payload).await
			}
		}
	};
}

level_action!(InputAction, INPUT);
level_action!(MixAction, MIX);

#[cfg(test)]
mod tests {
	use super::*;
	use crate::model::{Cell, Input, Mix};

	fn snapshot() -> Snapshot {
		Snapshot {
			protocol: 1,
			inputs: vec![Input {
				id: "sink_game".into(),
				label: "Game".into(),
				kind: "software".into(),
				icon: Some("game".into()),
				icon_color: Some("green".into()),
				volume: 80,
				muted: false,
				routes: [
					(
						"stream".to_owned(),
						Cell {
							enabled: true,
							send: 50,
							muted: false,
						},
					),
					(
						"chat".to_owned(),
						Cell {
							enabled: false,
							send: 100,
							muted: false,
						},
					),
				]
				.into(),
			}],
			mixes: vec![
				Mix {
					id: "stream".into(),
					label: "Stream".into(),
					icon: None,
					icon_color: None,
					volume: 90,
					muted: true,
				},
				Mix {
					id: "chat".into(),
					label: "Chat".into(),
					icon: None,
					icon_color: None,
					volume: 100,
					muted: false,
				},
			],
			..Default::default()
		}
	}

	fn settings(target: &str, mix: &str) -> LevelSettings {
		LevelSettings {
			target: target.into(),
			mix: mix.into(),
			..Default::default()
		}
	}

	#[test]
	fn an_input_fader_shows_the_input() {
		let v = view(INPUT, &settings("sink_game", ""), &snapshot()).expect("view");
		assert_eq!((v.volume, v.muted, v.live), (80, false, true));
		assert_eq!(v.caption, "All mixes");
		assert_eq!(v.tile.color, "#16a34a");
	}

	#[test]
	fn a_send_shows_its_cell() {
		let v = view(INPUT, &settings("sink_game", "stream"), &snapshot()).expect("view");
		assert_eq!((v.volume, v.live), (50, true));
		assert_eq!(v.caption, "→ Stream");

		let off = view(INPUT, &settings("sink_game", "chat"), &snapshot()).expect("view");
		assert!(!off.live, "an unrouted send is dimmed");
		assert_eq!(off.caption, "Chat · off");
	}

	#[test]
	fn a_mix_shows_its_level_and_mute() {
		let v = view(MIX, &settings("stream", ""), &snapshot()).expect("view");
		assert_eq!((v.volume, v.muted), (90, true));
		assert_eq!(
			v.tile.color, "#7c3aed",
			"mixes default to purple like the mixer"
		);
	}

	#[test]
	fn missing_targets_resolve_to_nothing() {
		assert!(view(INPUT, &settings("gone", ""), &snapshot()).is_none());
		assert!(view(INPUT, &settings("sink_game", "gone"), &snapshot()).is_none());
		assert!(view(MIX, &settings("", ""), &snapshot()).is_none());
	}

	#[test]
	fn touch_defaults_to_reset_and_supports_custom_level_or_mute() {
		// Existing settings without touch options also get the new default.
		let mut s: LevelSettings = serde_json::from_str(r#"{"target":"sink_game"}"#).unwrap();
		assert_eq!(s.touch_volume(), Some(100));
		assert_eq!(LevelSettings::default().touch_volume(), Some(100));
		s = serde_json::from_str(r#"{"touch":"reset","default_level":65}"#).unwrap();
		assert_eq!(s.touch_volume(), Some(65));
		s.default_level = Some(0);
		assert_eq!(s.touch_volume(), Some(0));
		s.default_level = Some(200);
		assert_eq!(s.touch_volume(), Some(100));
		s.touch = Some("mute".into());
		assert_eq!(s.touch_volume(), None);
		assert!(serde_json::from_str::<LevelSettings>(r#"{"default_level":-1}"#).is_err());
	}

	#[test]
	fn steps_are_bounded() {
		let mut s = LevelSettings::default();
		assert_eq!(s.step(), 5);
		s.step = Some(0);
		assert_eq!(s.step(), 1);
		s.step = Some(200);
		assert_eq!(s.step(), 50);
	}
}
