

https://github.com/user-attachments/assets/0cccc700-4311-4e1e-aded-40230454c056



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

## Usage
In a terminal:

```bash
git clone https://github.com/donishadsmith/Engram
cd Engram
cargo run --release
```

## Default Controls
The default controls are as follows but can be reconfigured in the menu.

| Keyboard    |    GBA     |     GB/GBC      |
|-------------|------------|-----------------|
| W           | Up         | Up              |
| A           | Left       | Left            |
| S           | Down       | Down            |
| D           | Right      | Right           |
| L           | A          | A               |
| K           | B          | B               |
| Enter       | Start      | Start           |
| Right Shift | Select     | Select          |
| I           | R          |                 |
| O           | L          |                 |

## Demo
https://github.com/user-attachments/assets/1eefa878-7702-4047-98b6-917dcba3d336

Captured with <a href="https://github.com/NickeManarin/ScreenToGif">ScreenToGif</a> and converted to mp4 with <a href="https://github.com/FFmpeg/FFmpeg">FFmpeg</a>.
## References

I relied heavily on the prior work of the emulation development community while developing my own emulators. In particular, [mGBA](https://github.com/mgba-emu/mgba), [NanoBoyAdvance](https://github.com/nba-emu/NanoBoyAdvance), [rustboyadvance-ng](https://github.com/michelhe/rustboyadvance-ng), [retroboy](https://github.com/smparsons/retroboy), [SameBoy](https://github.com/LIJI32/SameBoy), [Pan Docs](https://gbdev.io/pandocs/), and [GBATEK](https://problemkaputt.de/gbatek.htm) were incredibly helpful. The full list of emulators, documentation, and articles used is in [REFERENCES.md](REFERENCES.md).
