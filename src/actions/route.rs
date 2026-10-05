//! Route: send an input to a mix, or stop — one cell of the matrix.

use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{Settings, paint_key, paint_offline, paint_unset, sent};
use crate::client;
use crate::model::Snapshot;
use crate::render::{self, Tile, ToggleKey};

pub const UUID: &str = "us.echo.wavesink.route";

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct RouteSettings {
	pub input: String,
	pub mix: String,
}

static SETTINGS: Settings<RouteSettings> = Settings::new();

struct View {
	tile: Tile,
	label: String,
	caption: String,
	on: bool,
}

fn view(settings: &RouteSettings, snapshot: &Snapshot) -> Option<View> {
	let input = snapshot.input(&settings.input)?;
	let mix = snapshot.mix(&settings.mix)?;
	Some(View {
		tile: Tile {
			path: render::icon_path(input.icon.as_deref()),
			color: render::color_hex(input.icon_color.as_deref(), "blue"),
		},
		label: input.label.clone(),
		caption: format!("→ {}", mix.label),
		on: input.routes.get(&mix.id).is_some_and(|c| c.enabled),
	})
}

/// The first input and mix, so a new key is live straight away.
async fn default_target(instance: &Instance, settings: &mut RouteSettings) -> OpenActionResult<()> {
	if !settings.input.is_empty() && !settings.mix.is_empty() {
		return Ok(());
	}
	let firsts = client::with_snapshot(|s| {
		s.map(|s| {
			(
				s.inputs.first().map(|i| i.id.clone()),
				s.mixes.first().map(|m| m.id.clone()),
			)
		})
	});
	let Some((input, mix)) = firsts else {
		return Ok(());
	};
	if settings.input.is_empty() {
		settings.input = input.unwrap_or_default();
	}
	if settings.mix.is_empty() {
		settings.mix = mix.unwrap_or_default();
	}
	SETTINGS.put(&instance.instance_id, settings.clone());
	instance.set_settings(settings).await
}

pub async fn paint(instance: &Instance) -> OpenActionResult<()> {
	let Some(mut settings) = SETTINGS.get(&instance.instance_id) else {
		return Ok(());
	};
	if client::with_snapshot(|s| s.is_none()) {
		return paint_offline(instance).await;
	}
	default_target(instance, &mut settings).await?;
	let Some(view) = client::with_snapshot(|s| s.and_then(|s| view(&settings, s))) else {
		return paint_unset(instance, "a route").await;
	};
	paint_key(
		instance,
		render::toggle_key(&ToggleKey {
			tile: view.tile,
			label: &view.label,
			caption: &view.caption,
			badge: if view.on { "ON" } else { "OFF" },
			on: view.on,
		}),
	)
	.await
}

pub struct RouteAction;

#[async_trait]
impl Action for RouteAction {
	const UUID: &'static str = UUID;
	type Settings = RouteSettings;

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
		let known = client::with_snapshot(|s| s.and_then(|s| view(settings, s)).is_some());
		let ok = known
			&& client::call(
				"toggle_route",
				json!({ "input": settings.input, "mix": settings.mix, "field": "enabled" }),
			);
		sent(instance, ok).await
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
