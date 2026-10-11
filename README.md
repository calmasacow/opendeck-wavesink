# WaveSink for OpenDeck

Control the [WaveSink](https://github.com/calmasacow/WaveSink) mixer from your
Stream Deck. Mute your mic, ride the game volume on a dial, choose what goes to
your stream, and switch profiles. The keys show what WaveSink is doing right
now, and a change made in WaveSink's window shows up on the deck straight away.

## Actions

| Action | On a key | On a Stream Deck+ dial |
|---|---|---|
| **Input** | Mute, volume up or down, or jump to a level | Turn for volume, push to mute, tap to reset the level, with a live level meter |
| **Mix** | The same, for a whole mix | The same |
| **Route** | Send an input to a mix, or stop sending it (ON / OFF) | |
| **Solo** | Hear one input alone; press again to bring everything back | |
| **Profile** | Load a profile, or go to the next or previous one | Turn to pick a profile, push to load it |

An **Input** controls the input's own volume, which affects every mix, or just
its level in one mix. For example, you can turn the game down on your stream
without turning it down in your headphones. Choose this under **Controls** in
the action's settings.

On Input and Mix dials, touching the screen resets the controlled level to
100% by default. Set **Default level** to any value from 0–100%, or change
**Touch** to **Toggle mute**. Knob presses always toggle mute. Existing dial
actions also use the new touch default unless you select **Toggle mute**.

Keys use the same icons and colours as your inputs and mixes in WaveSink:

- a red frame means muted
- a green frame means a route or solo is on
- a dimmed key is an input that isn't sent to that mix right now

A new action starts on your first input or mix, so it works as soon as you
place it. You can change that in its settings.

## Requirements

- Linux with WaveSink **1.1.0 or newer**, running. It gives the plugin its
  connection.
- OpenDeck. If WaveSink isn't running, keys show "not running" and the plugin
  reconnects on its own when WaveSink starts.

## Privacy

The plugin talks only to WaveSink, through a private connection on your own
computer (`$XDG_RUNTIME_DIR/wavesink/control.sock`) that only your user account
can open. Nothing goes over the network.

## Help

Bugs and requests: [WaveSink issues](https://github.com/calmasacow/WaveSink/issues).

## Building from source

```bash
cargo test
cargo build --release
```

The plugin follows WaveSink's control-socket protocol, which is described in
WaveSink's [`docs/control-socket.md`](https://github.com/calmasacow/WaveSink/blob/main/docs/control-socket.md).

## License

GPL-3.0-or-later.
