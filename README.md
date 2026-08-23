# Engram

[![Run Tests](https://github.com/donishadsmith/Engram/actions/workflows/test.yml/badge.svg)](https://github.com/donishadsmith/Engram/actions/workflows/test.yml)
[![Publish](https://github.com/donishadsmith/Engram/actions/workflows/build.yml/badge.svg)](https://github.com/donishadsmith/Engram/actions/workflows/build.yml)

A Game Boy Color emulator (supports DMG games too) written in Rust. Game Boy Advance emulator is a work-in-progress and will eventually have a unified interface with the Game Boy emulator.

<table align="center">
  <tr>
    <td align="center">
      <img src="assets/shantae.gif" width="250">
      <br>Shantae
    </td>
    <td align="center">
      <img src="assets/pkmn_crystal.gif" width="250">
      <br>Pokemon Crystal
    </td>
    <td align="center">
      <img src="assets/road_rash.gif" width="250">
      <br>Road Rash
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="assets/pkmn_pinball.png" width="250">
      <br>Pokemon Pinball
    </td>
    <td align="center">
      <img src="assets/mario.png" width="250">
      <br>Super Mario Land
    </td>
    <td align="center">
      <img src="assets/loz.png" width="250">
      <br>Legend of Zelda: Oracle of Ages
    </td>
  </tr>
</table>

<p align="center"><i>Note: Pokemon Crystal battle sped up by dropping frames in <a href="https://github.com/NickeManarin/ScreenToGif">ScreenToGif</a>.</i></p>

## Controls

| Keyboard    | Button |
|-------------|--------|
| W           | Up     |
| A           | Left   |
| S           | Down   |
| D           | Right  |
| K           | A      |
| L           | B      |
| Enter       | Start  |
| Right Shift | Select |
| Esc         | Quit   |

`F1` key to dump data in sram to a .sav file for ROMs that are battery-backed.

## Usage
Either download the executable or clone the repository and run the following in a terminal:

``bash
git clone https://github.com/donishadsmith/Engram
cd Engram
cargo run
``
