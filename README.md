# SuperTuxShowdown

While there's a good amount of stuff here, nothing's ready for release. I'll update the README when that changes.

This project expects the use of the [Bevy CLI](https://github.com/TheBevyFlock/bevy_cli); visit its page for setup instructions.

There are (currently) two application packages: `super-tux-showdown` (in client) and `super-tux-showdown-frame-tool` (in frame-tool). To run one of them, use `bevy -p <name-of-package>`.

This repository is set up for use with VS Code and related editors. If anyone knows how to set up configuration for other editors, PRs are welcome.

The Cargo configuration in this repository expects Nightly Rust.

Have fun!

## Licenses

While I've been too lazy to put license files in this project, the code is dual-licensed under the MIT and Apache Version 2 licenses, as with most Rust projects.

Models thus far have been sourced from SuperTuxKart, with rigging and some slight alterations done by me. The revised models are released under no additional conditions; you are free to consider the modifications as public domain. Here are the source licenses:

* Tux ([assets/models/stk-tux.glb](client/assets/models/stk-tux.glb)): CC-BY-SA 3.0. Copyright 2015 Julian "XGhost" Schönbächler \<<info@creative-forge.ch>\>.
