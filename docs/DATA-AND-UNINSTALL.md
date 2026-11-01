# Data locations & clean uninstall

| What | Path |
|---|---|
| Device profile (versioned JSON, mode 0600, written atomically) | `~/.config/io.github.archdroid/profile.json` (a corrupt file is kept as `profile.json.corrupt`) |
| Install markers (`gapps_installed`, `houdini_installed`, `multitouch_installed`) and installer cache | `~/.cache/archdroid/` (`$XDG_CACHE_HOME`) |
| Application log (Settings → Application Log shows the exact path) | Tauri app log dir, e.g. `~/.local/share/io.github.wisangdg.archdroid/logs/` |
| Waydroid image, data and config | `/var/lib/waydroid`, `~/.local/share/waydroid` |
| Desktop entries for Android apps | `~/.local/share/applications/waydroid.*.desktop` |

## Markers
Markers are only a shortcut for when the container is offline:

- A successful install writes `pending`; the UI shows **Unverified** until a
  live check against the running container rewrites it as `verified`.
- A live check that does not find the add-on deletes the marker.
- A reinstall that obtained root and then failed deletes the marker (the old
  install may be half-replaced); a refused password prompt keeps it.
- Init/Wipe clear all markers. Markers from older builds (`installed`) count as
  `pending`.
- If a marker cannot be written after an install, the terminal shows a warning.

## Clean uninstall
1. In ArchDroid, Settings → Factory Reset (or `sudo waydroid session stop && sudo waydroid-container stop`).
2. Remove the package: `pacman -R archdroid-bin` (Arch), `apt remove arch-droid` (Debian/Ubuntu), `dnf remove arch-droid` (Fedora). The `.deb`/`.rpm` package name is `arch-droid`, derived from the product name.
3. Delete user data: `rm -rf ~/.config/io.github.archdroid ~/.cache/archdroid ~/.local/share/io.github.wisangdg.archdroid`.
4. Optional, removes all Android data: `sudo rm -rf /var/lib/waydroid ~/.local/share/waydroid`.

Back up files from Android shared storage before step 1 or 4.
