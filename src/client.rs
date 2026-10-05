//! The connection to WaveSink's control socket.
//!
//! One long-lived connection, re-established every couple of seconds while
//! WaveSink is not running. The latest snapshot and meter frame are kept
//! here for the actions to render from; every change re-renders them.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, RwLock};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

use crate::model::{Levels, PROTOCOL, Snapshot};

const RECONNECT: Duration = Duration::from_secs(2);

#[derive(Default)]
struct Shared {
	snapshot: Option<Snapshot>,
	levels: Levels,
}

static SHARED: LazyLock<RwLock<Shared>> = LazyLock::new(Default::default);
static OUTBOX: Mutex<Option<mpsc::UnboundedSender<String>>> = Mutex::new(None);
static WANT_LEVELS: AtomicBool = AtomicBool::new(false);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// The same resolution WaveSink uses to bind it.
fn socket_path() -> Option<PathBuf> {
	if let Some(path) = std::env::var_os("WAVESINK_CONTROL_SOCKET") {
		return Some(PathBuf::from(path));
	}
	let base = match std::env::var_os("XDG_RUNTIME_DIR") {
		Some(dir) => PathBuf::from(dir).join("wavesink"),
		None => {
			use std::os::unix::fs::MetadataExt;
			let uid = std::fs::metadata("/proc/self").ok()?.uid();
			std::env::temp_dir().join(format!("wavesink-{uid}"))
		}
	};
	Some(base.join("control.sock"))
}

/// Read the current state; `None` while WaveSink is unreachable.
pub fn with_snapshot<T>(f: impl FnOnce(Option<&Snapshot>) -> T) -> T {
	let shared = SHARED.read().unwrap_or_else(|e| e.into_inner());
	f(shared.snapshot.as_ref())
}

pub fn with_levels<T>(f: impl FnOnce(&Levels) -> T) -> T {
	let shared = SHARED.read().unwrap_or_else(|e| e.into_inner());
	f(&shared.levels)
}

pub fn connected() -> bool {
	OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

fn send_line(line: String) -> bool {
	match OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
		Some(tx) => tx.send(line).is_ok(),
		None => false,
	}
}

/// Send one request. `false` when WaveSink isn't connected, so the action
/// can flash an alert.
pub fn call(method: &str, params: Value) -> bool {
	let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
	send_line(json!({ "id": id, "method": method, "params": params }).to_string())
}

fn subscribe_line() -> String {
	json!({
		"id": 0,
		"method": "subscribe",
		"params": { "state": true, "levels": WANT_LEVELS.load(Ordering::Relaxed) },
	})
	.to_string()
}

/// Meters cost WaveSink a stream per input; only ask while a dial shows one.
pub fn set_want_levels(want: bool) {
	if WANT_LEVELS.swap(want, Ordering::Relaxed) != want {
		send_line(subscribe_line());
		if !want {
			SHARED.write().unwrap_or_else(|e| e.into_inner()).levels = Levels::default();
		}
	}
}

/// Connect, follow, reconnect: runs for the life of the plugin.
pub async fn run() {
	let mut logged_offline = false;
	loop {
		match socket_path() {
			Some(path) => match UnixStream::connect(&path).await {
				Ok(stream) => {
					logged_offline = false;
					log::info!("connected to WaveSink at {}", path.display());
					session(stream).await;
					log::info!("WaveSink disconnected");
				}
				Err(error) if !logged_offline => {
					log::info!(
						"WaveSink not reachable at {} ({error}); retrying",
						path.display()
					);
					logged_offline = true;
				}
				Err(_) => {}
			},
			None if !logged_offline => {
				log::warn!("no runtime directory to find WaveSink's socket in");
				logged_offline = true;
			}
			None => {}
		}
		tokio::time::sleep(RECONNECT).await;
	}
}

async fn session(stream: UnixStream) {
	let (read, mut write) = stream.into_split();
	let (tx, mut rx) = mpsc::unbounded_channel::<String>();
	// Queued ahead of anything an action sends.
	let _ = tx.send(json!({ "id": 0, "method": "hello" }).to_string());
	let _ = tx.send(subscribe_line());
	*OUTBOX.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

	let writer = tokio::spawn(async move {
		while let Some(mut line) = rx.recv().await {
			line.push('\n');
			if write.write_all(line.as_bytes()).await.is_err() {
				break;
			}
		}
	});

	let mut lines = BufReader::new(read).lines();
	while let Ok(Some(line)) = lines.next_line().await {
		let Ok(message) = serde_json::from_str::<Value>(&line) else {
			log::warn!("unreadable message from WaveSink: {line}");
			continue;
		};
		match message.get("event").and_then(Value::as_str) {
			Some("state") => match serde_json::from_value::<Snapshot>(message["data"].clone()) {
				Ok(snapshot) => {
					if snapshot.protocol != PROTOCOL {
						log::warn!(
							"WaveSink speaks control protocol {}, this plugin {PROTOCOL}",
							snapshot.protocol
						);
					}
					SHARED.write().unwrap_or_else(|e| e.into_inner()).snapshot = Some(snapshot);
					crate::actions::refresh_all().await;
				}
				Err(error) => log::error!("bad state from WaveSink: {error}"),
			},
			Some("levels") => {
				if let Ok(levels) = serde_json::from_value::<Levels>(message["data"].clone()) {
					SHARED.write().unwrap_or_else(|e| e.into_inner()).levels = levels;
					crate::actions::refresh_meters().await;
				}
			}
			Some(other) => log::debug!("ignoring WaveSink event {other}"),
			None => {
				if let Some(error) = message.get("error").and_then(Value::as_str) {
					log::warn!("WaveSink refused request {}: {error}", message["id"]);
				}
			}
		}
	}

	*OUTBOX.lock().unwrap_or_else(|e| e.into_inner()) = None;
	writer.abort();
	{
		let mut shared = SHARED.write().unwrap_or_else(|e| e.into_inner());
		shared.snapshot = None;
		shared.levels = Levels::default();
	}
	crate::actions::refresh_all().await;
}
