//! The actions, and what they share: per-instance settings, a paint cache so
//! an unchanged key is never resent, and the lists the property inspectors
//! pick from.

pub mod level;
pub mod profile;
pub mod route;
pub mod solo;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use openaction::{Instance, OpenActionResult, visible_instances};
use serde_json::{Value, json};

use crate::client;
use crate::render;

/// Settings by instance. The SDK hands settings to event handlers only, and
/// a state push re-renders every key.
pub struct Settings<S>(LazyLock<Mutex<HashMap<String, S>>>);

impl<S: Clone> Settings<S> {
	pub const fn new() -> Self {
		Self(LazyLock::new(Default::default))
	}

	pub fn put(&self, id: &str, settings: S) {
		self.0
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.insert(id.to_owned(), settings);
	}

	pub fn get(&self, id: &str) -> Option<S> {
		self.0
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.get(id)
			.cloned()
	}

	pub fn remove(&self, id: &str) {
		self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
	}
}

/// The last image or feedback sent per instance.
static PAINTED: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);
/// The last meter value per dial, so silence isn't resent 15 times a second.
static METERS: LazyLock<Mutex<HashMap<String, u8>>> = LazyLock::new(Default::default);
/// Instances whose property inspector is open.
static INSPECTING: LazyLock<Mutex<Vec<String>>> = LazyLock::new(Default::default);

fn changed(id: &str, content: String) -> bool {
	let mut painted = PAINTED.lock().unwrap_or_else(|e| e.into_inner());
	if painted.get(id) == Some(&content) {
		return false;
	}
	painted.insert(id.to_owned(), content);
	true
}

/// Forget what an instance shows, so the next paint goes out whole: it just
/// appeared, or went away.
pub fn forget(id: &str) {
	PAINTED.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
	METERS.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
}

pub fn is_dial(instance: &Instance) -> bool {
	instance.controller == "Encoder"
}

pub async fn paint_key(instance: &Instance, svg: String) -> OpenActionResult<()> {
	if changed(&instance.instance_id, svg.clone()) {
		instance
			.set_image(Some(render::data_url(&svg)), None)
			.await?;
	}
	Ok(())
}

pub async fn paint_dial(instance: &Instance, feedback: Value) -> OpenActionResult<()> {
	if changed(&instance.instance_id, feedback.to_string()) {
		if let Some(meter) = feedback.pointer("/meter/value").and_then(Value::as_u64) {
			METERS
				.lock()
				.unwrap_or_else(|e| e.into_inner())
				.insert(instance.instance_id.clone(), meter as u8);
		}
		instance.set_feedback(&feedback).await?;
	}
	Ok(())
}

/// Move only the meter, between state pushes.
pub async fn paint_meter(instance: &Instance, value: u8) -> OpenActionResult<()> {
	{
		let mut meters = METERS.lock().unwrap_or_else(|e| e.into_inner());
		if meters.get(&instance.instance_id) == Some(&value) {
			return Ok(());
		}
		meters.insert(instance.instance_id.clone(), value);
	}
	instance
		.set_feedback(&json!({ "meter": { "value": value } }))
		.await
}

/// The shared "not connected" face.
pub async fn paint_offline(instance: &Instance) -> OpenActionResult<()> {
	if is_dial(instance) {
		paint_dial(instance, render::dial_notice("WaveSink", "Offline")).await
	} else {
		paint_key(instance, render::notice_key("WaveSink", "not running")).await
	}
}

pub async fn paint_unset(instance: &Instance, what: &str) -> OpenActionResult<()> {
	if is_dial(instance) {
		paint_dial(
			instance,
			render::dial_notice("WaveSink", &format!("Pick {what}")),
		)
		.await
	} else {
		paint_key(
			instance,
			render::notice_key("Set up", &format!("pick {what}")),
		)
		.await
	}
}

/// A press that couldn't reach WaveSink, or named something it no longer
/// has.
pub async fn sent(instance: &Instance, ok: bool) -> OpenActionResult<()> {
	if ok {
		Ok(())
	} else {
		instance.show_alert().await
	}
}

/// What the inspectors list: everything WaveSink has, by id and label.
pub fn choices() -> Value {
	client::with_snapshot(|snapshot| match snapshot {
		None => json!({ "event": "choices", "connected": false }),
		Some(s) => json!({
			"event": "choices",
			"connected": true,
			"inputs": s.inputs.iter().map(|i| json!({ "id": i.id, "label": i.label, "kind": i.kind })).collect::<Vec<_>>(),
			"mixes": s.mixes.iter().map(|m| json!({ "id": m.id, "label": m.label })).collect::<Vec<_>>(),
			"profiles": s.profiles,
		}),
	})
}

pub async fn inspector_opened(instance: &Instance) -> OpenActionResult<()> {
	{
		let mut open = INSPECTING.lock().unwrap_or_else(|e| e.into_inner());
		if !open.contains(&instance.instance_id) {
			open.push(instance.instance_id.clone());
		}
	}
	instance.send_to_property_inspector(choices()).await
}

pub fn inspector_closed(instance: &Instance) {
	INSPECTING
		.lock()
		.unwrap_or_else(|e| e.into_inner())
		.retain(|id| id != &instance.instance_id);
}

/// An inspector asks again when it loads, in case it missed the first send.
pub async fn inspector_message(instance: &Instance, payload: &Value) -> OpenActionResult<()> {
	if payload.get("request").and_then(Value::as_str) == Some("choices") {
		instance.send_to_property_inspector(choices()).await?;
	}
	Ok(())
}

async fn refresh_inspectors() {
	let open: Vec<String> = INSPECTING.lock().unwrap_or_else(|e| e.into_inner()).clone();
	if open.is_empty() {
		return;
	}
	let choices = choices();
	for id in open {
		if let Some(instance) = openaction::get_instance(id).await {
			let _ = instance.send_to_property_inspector(choices.clone()).await;
		}
	}
}

/// Ask for meters only while some dial shows one.
pub fn update_meter_interest() {
	client::set_want_levels(level::wants_meters());
}

async fn each(uuid: &'static str, paint: impl AsyncFn(&Instance) -> OpenActionResult<()>) {
	for instance in visible_instances(uuid).await {
		if let Err(error) = paint(&instance).await {
			log::warn!("painting {uuid}: {error}");
		}
	}
}

/// WaveSink's state changed, or it came or went: repaint everything.
pub async fn refresh_all() {
	each(level::INPUT, level::paint).await;
	each(level::MIX, level::paint).await;
	each(route::UUID, route::paint).await;
	each(solo::UUID, solo::paint).await;
	each(profile::UUID, profile::paint).await;
	refresh_inspectors().await;
}

/// A meter frame: only dials move.
pub async fn refresh_meters() {
	each(level::INPUT, level::paint_meter).await;
	each(level::MIX, level::paint_meter).await;
}
