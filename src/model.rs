//! WaveSink's control-socket snapshot (protocol 1), as the plugin reads it.

use std::collections::BTreeMap;

use serde::Deserialize;

pub const PROTOCOL: u32 = 1;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Cell {
	pub enabled: bool,
	pub send: u8,
	pub muted: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Input {
	pub id: String,
	pub label: String,
	pub kind: String,
	pub icon: Option<String>,
	pub icon_color: Option<String>,
	pub volume: u8,
	pub muted: bool,
	#[serde(default)]
	pub routes: BTreeMap<String, Cell>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mix {
	pub id: String,
	pub label: String,
	pub icon: Option<String>,
	pub icon_color: Option<String>,
	pub volume: u8,
	pub muted: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Snapshot {
	pub protocol: u32,
	pub inputs: Vec<Input>,
	pub mixes: Vec<Mix>,
	pub solo: Option<String>,
	pub profiles: Vec<String>,
	pub active_profile: Option<String>,
}

impl Snapshot {
	pub fn input(&self, id: &str) -> Option<&Input> {
		self.inputs.iter().find(|i| i.id == id)
	}

	pub fn mix(&self, id: &str) -> Option<&Mix> {
		self.mixes.iter().find(|m| m.id == id)
	}
}

/// Linear peaks, 0..1.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Levels {
	#[serde(default)]
	pub inputs: BTreeMap<String, f32>,
	#[serde(default)]
	pub mixes: BTreeMap<String, f32>,
}
