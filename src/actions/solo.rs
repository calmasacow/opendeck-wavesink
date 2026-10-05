//! Solo: hear one input alone. Pressing again puts everyone's mutes back,
//! exactly like right-clicking the input's icon in WaveSink.

use openaction::{Action, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{Settings, paint_key, paint_offline, paint_unset, sent};
use crate::client;
use crate::render::{self, Tile, ToggleKey};

pub const UUID: &str = "us.echo.wavesink.solo";

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct SoloSettings {
	pub input: String,
}

static SETTINGS: Settings<SoloSettings> = Settings::new();

pub async fn paint(instance: &Instance) -> OpenActionResult<()> {
	let Some(mut settings) = SETTINGS.get(&instance.instance_id) else {
		return Ok(());
	};
	if client::with_snapshot(|s| s.is_none()) {
		return paint_offline(instance).await;
	}
	if settings.input.is_empty()
		&& let Some(first) =
			client::with_snapshot(|s| s.and_then(|s| s.inputs.first().map(|i| i.id.clone())))
	{
		settings.input = first;
		SETTINGS.put(&instance.instance_id, settings.clone());
		instance.set_settings(&settings).await?;
	}
	let view = client::with_snapshot(|s| {
		s.and_then(|s| {
			let input = s.input(&settings.input)?;
			let soloed = s.solo.as_deref() == Some(input.id.as_str());
			let caption = match (&s.solo, soloed) {
				(_, true) => "soloed".to_owned(),
				(Some(other), false) => format!(
					"{} solo",
					s.input(other).map(|i| i.label.as_str()).unwrap_or("other")
				),
				(None, false) => "press to solo".to_owned(),
			};
			let tile = Tile {
				path: render::icon_path(input.icon.as_deref()),
				color: render::color_hex(input.icon_color.as_deref(), "blue"),
			};
			Some((tile, input.label.clone(), caption, soloed))
		})
	});
	let Some((tile, label, caption, on)) = view else {
		return paint_unset(instance, "an input").await;
	};
	paint_key(
		instance,
		render::toggle_key(&ToggleKey {
			tile,
			label: &label,
			caption: &caption,
			badge: "SOLO",
			on,
		}),
	)
	.await
}

pub struct SoloAction;

#[async_trait]
impl Action for SoloAction {
	const UUID: &'static str = UUID;
	type Settings = SoloSettings;

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
		let known =
			client::with_snapshot(|s| s.is_some_and(|s| s.input(&settings.input).is_some()));
		let ok = known && client::call("toggle_solo", json!({ "input": settings.input }));
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
