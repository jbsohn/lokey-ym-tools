# lokey-ym-tools

A comprehensive developer toolchain for compiling, auditing, and auditioning sound sequences and music streams targetting the Yamaha YM-2149 Programmable Sound Generator (PSG).

## Crates in the Workspace

- **`ym-core`**: The foundational library containing the platform-agnostic `DeltaCompiler`, YM register configurations, frame structures, and the real-time audio playback engine.
- **`ym-sfx`**: CLI toolchain for rendering JSON/CSV sound effect definitions into compiled `.yfx` binary payloads and auditioning them in real-time.
- **`ym-song`**: CLI toolchain for music song sequence rendering (compiling into `.ysg` format) and playing standard YM chiptune formats.

---

## Getting Started

### Prerequisites
Ensure you have the Rust toolchain installed. Since auditioning plays audio directly through your speakers, `cpal` will bind to your system's default audio host (ALSA on Linux, CoreAudio on macOS, WASAPI on Windows).

### Usage

#### 1. Play standard YM Chiptune files:
```bash
cargo run --bin ym-song -- play --input path/to/music.ym
```

#### 2. Play custom JSON sound effects:
```bash
cargo run --bin ym-sfx -- play --input tests/fixtures/test_sfx.json
```

#### 3. Render custom JSON sequences into compiled YM-2149 binary payloads:
```bash
cargo run --bin ym-sfx -- render --input tests/fixtures/test_sfx.json --output tests/fixtures/laser.yfx
```

---

## Acknowledgements & Credits

This toolset leverages the excellent **`ym2149-rs`** ecosystem developed by [slippyex](https://github.com/slippyex) for low-level emulation and chiptune parsing:

- **[`ym2149`](https://crates.io/crates/ym2149)**: Provides the cycle-accurate Yamaha YM-2149 PSG emulator core.
- **[`ym2149-common`](https://crates.io/crates/ym2149-common)**: Outlines player traits and frequency helper types.
- **[`ym2149-ym-replayer`](https://crates.io/crates/ym2149-ym-replayer)**: Performs loader, parser, and decompressed vbl-sync playback logic for legacy Atari ST `.ym` music formats.

We extend our deep gratitude to the authors of these crates for providing the cycle-accurate emulation engine that powers the real-time auditioning tools in this codebase.
