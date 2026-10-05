//! Profile: load one WaveSink profile, or step through them.
//!
//! On a dial, rotating picks a profile without loading it (a profile load
//! rebuilds the graph, so scrolling past one mustn't); pushing loads it.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{Settings, is_dial, paint_dial, paint_key, paint_offline, sent};
use crate::client;
use crate::render::{self, Dial, Tile, ToggleKey};

pub const UUID: &str = "us.echo.wavesink.profile";

const TILE: Tile = Tile {
	path: render::PROFILE_ICON,
	color: "#475569",
};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct ProfileSettings {
	/// "load" (default), "next" or "previous".
	pub mode: String,
	pub profile: String,
}

static SETTINGS: Settings<ProfileSettings> = Settings::new();
/// The profile a dial has scrolled to but not loaded yet.
static PICKED: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);

fn picked(id: &str) -> Option<String> {
	PICKED
		.lock()
		.unwrap_or_else(|e| e.into_inner())
		.get(id)
		.cloned()
}

fn set_picked(id: &str, name: Option<String>) {
	let mut picked = PICKED.lock().unwrap_or_else(|e| e.into_inner());
	match name {
		Some(name) => picked.insert(id.to_owned(), name),
		None => picked.remove(id),
	};
}

pub async fn paint(instance: &Instance) -> OpenActionResult<()> {
	let Some(settings) = SETTINGS.get(&instance.instance_id) else {
		return Ok(());
	};
	let Some((profiles, active)) =
		client::with_snapshot(|s| s.map(|s| (s.profiles.clone(), s.active_profile.clone())))
	else {
		return paint_offline(instance).await;
	};
	let active_name = active.clone().unwrap_or_else(|| "none".into());

	if is_dial(instance) {
		// A pick that was loaded, or that vanished, is done with.
		let pick = picked(&instance.instance_id)
			.filter(|p| profiles.contains(p) && Some(p) != active.as_ref());
		if pick.is_none() {
			set_picked(&instance.instance_id, None);
		}
		let (title, value, live) = match &pick {
			Some(p) => ("Push to load", p.clone(), true),
			None => ("Profile", active_name, active.is_some()),
		};
		return paint_dial(
			instance,
			render::dial(&Dial {
				tile: TILE,
				title,
				volume: None,
				value: Some(value),
				muted: false,
				live,
				meter: None,
			}),
		)
		.await;
	}

	let key = match settings.mode.as_str() {
		"next" | "previous" => ToggleKey {
			tile: TILE,
			label: if settings.mode == "next" {
				"Next"
			} else {
				"Previous"
			},
			caption: &active_name,
			badge: if settings.mode == "next" {
				"▶"
			} else {
				"◀"
			},
			on: false,
		},
		_ => {
			let on = !settings.profile.is_empty()
				&& active.as_deref() == Some(settings.profile.as_str());
			let missing = !settings.profile.is_empty() && !profiles.contains(&settings.profile);
			ToggleKey {
				tile: TILE,
				label: if settings.profile.is_empty() {
					"Profile"
				} else {
					&settings.profile
				},
				caption: if settings.profile.is_empty() {
					"pick a profile"
				} else if missing {
					"not found"
				} else if on {
					"active"
				} else {
					"press to load"
				},
				badge: if on { "ON" } else { "" },
				on,
			}
		}
	};
	paint_key(instance, render::toggle_key(&key)).await
}

fn cycle(direction: i32) -> bool {
	client::call("cycle_profile", json!({ "direction": direction }))
}

fn load(name: &str) -> bool {
	client::with_snapshot(|s| s.is_some_and(|s| s.profiles.iter().any(|p| p == name)))
		&& client::call("load_profile", json!({ "name": name }))
}

pub struct ProfileAction;

#[async_trait]
impl Action for ProfileAction {
	const UUID: &'static str = UUID;
	type Settings = ProfileSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		super::forget(&instance.instance_id);
		SETTINGS.put(&instance.instance_id, settings.clone());
		paint(instance).await
	}

	async fn will_disappear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		SETTINGS.remove(&instance.instance_id);
		set_picked(&instance.instance_id, None);
		super::forget(&instance.instance_id);
		super::inspector_closed(instance);
		Ok(())
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		SETTINGS.put(&instance.instance_id, settings.clone());
		paint(instance).await
	}

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let ok = match settings.mode.as_str() {
			"next" => cycle(1),
			"previous" => cycle(-1),
			_ => load(&settings.profile),
		};
		sent(instance, ok).await
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		let Some((profiles, active)) =
			client::with_snapshot(|s| s.map(|s| (s.profiles.clone(), s.active_profile.clone())))
		else {
			return sent(instance, false).await;
		};
		if profiles.is_empty() {
			return sent(instance, false).await;
		}
		let from = picked(&instance.instance_id).or(active);
		let at = from
			.and_then(|f| profiles.iter().position(|p| *p == f))
			.unwrap_or(0) as i64;
		let next = (at + i64::from(ticks)).rem_euclid(profiles.len() as i64) as usize;
		set_picked(&instance.instance_id, Some(profiles[next].clone()));
		paint(instance).await
	}

	async fn dial_down(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		match picked(&instance.instance_id) {
			Some(name) => sent(instance, load(&name)).await,
			None => Ok(()),
		}
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
