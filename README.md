# Engram

[![Run Tests](https://github.com/donishadsmith/Engram/actions/workflows/test.yml/badge.svg)](https://github.com/donishadsmith/Engram/actions/workflows/test.yml)
[![Publish](https://github.com/donishadsmith/Engram/actions/workflows/build.yml/badge.svg)](https://github.com/donishadsmith/Engram/actions/workflows/build.yml)

A Gameboy Advance & Game Boy Color emulator (supports DMG games too) written in Rust.

<table align="center">
  <tr>
    <td align="center">
      <img src="assets/mmbn6.gif" width="250">
      <br>Megaman Battle Network 6 (GBA)
    </td>
    <td align="center">
      <img src="assets/pkmn_emerald.gif" width="250">
      <br>Pokemon Emerald (GBA)
    </td>
    <td align="center">
      <img src="assets/hamtaro.gif" width="250">
      <br>Hamtaro Ham Ham Heartbreak (GBA)
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="assets/shantae.png" width="250">
      <br>Shantae (GBC)
    </td>
    <td align="center">
      <img src="assets/pkmn_crystal.png" width="250">
      <br>Pokemon Crystal (GBC)
    </td>
    <td align="center">
      <img src="assets/mario.png" width="250">
      <br>Super Mario Land (GB)
    </td>
  </tr>
</table>

<p align="center"><i>Gifs captured with built-in gif recorder.</i></p>

<table align="center">
  <tr>
    <td align="center">
      <img src="assets/audio.png" width="500">
      <br>GBA Audio Debugger
    </td>
    <td align="center">
      <img src="assets/video.png" width="500">
      <br>GBA Video Debugger
  </tr>
</table>

## Usage
```bash
git clone https://github.com/donishadsmith/Engram
cd Engram
cargo run --release
```

## Demo
https://github.com/user-attachments/assets/de25befe-c87b-4c96-b198-8b29d849e68b

Captured with <a href="https://github.com/NickeManarin/ScreenToGif">ScreenToGif</a> and converted to mp4 with <a href="https://github.com/FFmpeg/FFmpeg">FFmpeg</a>.

## References

I relied heavily on the prior work of the emulation development community while developing my own emulators. In particular, [mGBA](https://github.com/mgba-emu/mgba), [NanoBoyAdvance](https://github.com/nba-emu/NanoBoyAdvance), [rustboyadvance-ng](https://github.com/michelhe/rustboyadvance-ng), [retroboy](https://github.com/smparsons/retroboy), [SameBoy](https://github.com/LIJI32/SameBoy), [Pan Docs](https://gbdev.io/pandocs/), and [GBATEK](https://problemkaputt.de/gbatek.htm) were incredibly helpful. The full list of emulators, documentation, and articles used is in [REFERENCES.md](REFERENCES.md).
