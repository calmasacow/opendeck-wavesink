//! WaveSink for OpenDeck: drives the WaveSink mixer over its control socket
//! (see WaveSink's `docs/control-socket.md`) and shows its live state on the
//! keys and dials.

mod actions;
mod client;
mod model;
mod render;

use openaction::*;

#[tokio::main]
async fn main() -> OpenActionResult<()> {
	{
		use simplelog::*;
		if let Err(error) = TermLogger::init(
			LevelFilter::Info,
			Config::default(),
			TerminalMode::Stdout,
			ColorChoice::Never,
		) {
			eprintln!("Logger initialization failed: {}", error);
		}
	}

	register_action(actions::level::InputAction).await;
	register_action(actions::level::MixAction).await;
	register_action(actions::route::RouteAction).await;
	register_action(actions::solo::SoloAction).await;
	register_action(actions::profile::ProfileAction).await;

	tokio::spawn(client::run());

	run(std::env::args().collect()).await
}
