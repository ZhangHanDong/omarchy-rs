# Rust Lang

A personal, unofficial Omarchy theme inspired by the Rust programming
language: forged charcoal surfaces, oxidized copper, restrained cyan signals,
and Rust orange focus accents.

The Rust Forge wallpaper is original generated artwork. This theme is not
affiliated with or endorsed by the Rust Project or Rust Foundation.

## Contents

- `colors.toml`: dark Rust-orange, copper, charcoal, and warm-neutral palette.
- `hyprland.lua`: orange-to-copper active borders and muted inactive borders.
- `icons.theme`: `Yaru-wartybrown-dark` icon selection.
- `backgrounds/rust-forge-4k.png`: original 3840×2160 wallpaper.

## Install

From an `omarchy-rs` checkout:

```bash
test ! -e "$HOME/.config/omarchy/themes/rust-lang"
mkdir -p "$HOME/.config/omarchy/themes"
cp -a themes/rust-lang "$HOME/.config/omarchy/themes/rust-lang"
omarchy theme set rust-lang
```

The first command deliberately stops the copy instructions when a theme with
the same name already exists. Back up or rename that directory yourself before
installing this version.

## Uninstall

Switch to another theme first, then remove the user-owned directory:

```bash
omarchy theme set catppuccin
rm -r "$HOME/.config/omarchy/themes/rust-lang"
```

Never install this theme into `/usr/share/omarchy/themes`; that directory is
owned by the Omarchy package and is replaced by system updates.

## Trademark and licensing

This is an unofficial Rust-inspired design and does not imply endorsement. See
the [Rust Logo Policy and Media Guide](https://foundation.rust-lang.org/policies/logo-policy-and-media-guide/).

The theme files and original Rust Forge artwork are distributed under the
repository's [MIT License](../../LICENSE).
