# Bandtools — Guitar Studio

A simple desktop studio for guitar players (Rust, egui), built as a guitar-focused take on GarageBand.

- **Amp & Cabinet**: 10 amp models, 8 cabinets, 3 microphones, with familiar knob layouts and neutral names.
- **Pedalboard**: up to 14 stomp boxes from 16 pedal types (drive, fuzz, modulation, delay, reverb…).
- **Guitar Tab**: tab editor with playback, text-tab import/export, and tuning presets.
- **Stem Splitter**: paste a YouTube link or drop a file; it is split into guitar, vocals, drums, bass, piano and other, then added to the timeline.
- **AI Assistant**: describe a sound and a local open-source LLM sets up the amp and pedals; the tab recognizer turns a track (best: the guitar stem) into tab.
- **Tuner**: in tune within ±3 cents, ±50 cent needle, Standard / Open D / Dropped D / Tune Down Whole Step.

## Run

```sh
cargo run --release
```

## Free helper tools (installed from the app with "Install / update helpers")

| Tool | Used for |
|---|---|
| yt-dlp + ffmpeg (Homebrew) | YouTube audio download |
| Demucs `htdemucs_6s` (pip) | 6-stem splitting, including guitar |
| Basic Pitch (pip) | polyphonic audio-to-notes for tab |
| Ollama (Homebrew) + `qwen2.5:7b` | local LLM for AI tone setup; the app starts it and downloads the model on first use |

Everything runs on your computer; nothing is uploaded.

## Tests

```sh
cargo test
# end-to-end (network, a few minutes):
BANDTOOLS_TEST_SONG="<YouTube link or .wav>" cargo test -- --ignored split_
BANDTOOLS_TEST_MODEL=qwen2.5:7b cargo test -- --ignored ask_
```

`MECHANICS.md` and `mechanics.json` list the values the app uses and where each one came from.
