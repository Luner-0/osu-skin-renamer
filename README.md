# osu-skin-renamer

List, rename and tag osu!**lazer** skins, directly in the `client.realm`
database where lazer stores skin names.

**osu! must be closed while renaming.** A backup of `client.realm` is made
before every write.

## Download

Grab the latest build from [Releases](https://github.com/Luner-0/osu-skin-renamer/releases):

| File | Platform |
|---|---|
| `osu-skin-renamer-<version>-Windows.exe` | Windows: a single ~8 MB exe, no install needed. Uses the WebView2 runtime that ships with Windows 10/11. SmartScreen may warn because it's unsigned (*More info → Run anyway*). |
| `osu-skin-renamer-<version>-Linux` | Linux: `chmod +x osu-skin-renamer-*-Linux && ./osu-skin-renamer-*-Linux`. Needs `webkit2gtk-4.1` (Arch: `sudo pacman -S webkit2gtk-4.1`). |

## Using the app

The window lists all your skins. Renaming works like Windows Explorer:
double-click a name (or select it and press **F2**), then **Enter** to accept
or **Esc** to cancel. **Right-click** opens a menu with *Rename*, *Edit
metadata…* and *Delete* (also the **Del** key). Deleting asks first and only marks the skin as
deleted; osu! removes it for good on its next start. The box at the top
filters the list.

- **Metadata:** *Edit metadata…* edits a skin's name and creator, and shows
  its ID and type. Like renames, the changes are pending until you save.
- **Save / undo:** renames are only written to osu! when you press **Save**
  (**Ctrl+S**), all at once with a single backup. Unsaved skins are shown
  in pink. **Undo** (**Ctrl+Z**) and **Redo** (**Ctrl+Y**) step through
  your changes.
- **Tags:** add prefix and suffix tags (e.g. `[NM]`) in the panel on the
  right. Drag a tag onto a skin to add it; click a tag to toggle it on the
  selected skin.
- **Multi-select:** **Ctrl+click** (or Alt+click) to select several skins; tags then apply
  to all of them.

### Where things live

| | Windows | Linux |
|---|---|---|
| Skin database | `%APPDATA%\osu\client.realm` | `~/.local/share/osu/client.realm` (or `$XDG_DATA_HOME/osu`) |
| Backups | `%APPDATA%\osu-skin-renamer\backups` | `~/.config/osu-skin-renamer/backups` |

Only the 2 newest backups are kept.

The app finds osu! automatically, including a custom location set in
lazer's settings (*Change folder location*, read from `storage.ini`) and
Flatpak lazer (`~/.var/app/sh.ppy.osu/data/osu`). If it still can't find
it, click **📁 osu! folder** and pick the folder that contains
`client.realm`; the app remembers it.

## Command line

Run the exe with a command instead of no arguments:

```
osu-skin-renamer list
```
Shows all skins with their IDs. Every command accepts `--db <path>` (the
`client.realm` file or the folder containing it) to use a different database.

### Edit or delete a single skin

```
osu-skin-renamer set --id <guid> --name "New name"
osu-skin-renamer set --id <guid> --creator "New creator"
osu-skin-renamer delete --id <guid>
```

### Bulk rename from a list (recommended)

```
osu-skin-renamer export
```
Writes `names.tsv` (ID → name). Edit the names in the second column
(tab-separated); lines you don't want to change can be deleted or left as
they are. Then:

```
osu-skin-renamer apply --dry-run   # preview
osu-skin-renamer apply             # apply
```

### Bulk rename with a regex

```
osu-skin-renamer rename --match "^Aristia" --replace "ARST" --dry-run
osu-skin-renamer rename --match "\s+$" --replace ""        # strip trailing spaces
```

## Building from source

Written in Rust. The UI is `src/index.html` shown in the system webview
(WebView2 / WebKitGTK); the database is accessed through realm-core's C API,
compiled from source and linked statically.

```
git clone --depth 1 --branch v20.1.2 https://github.com/realm/realm-core.git vendor/realm-core
git -C vendor/realm-core submodule update --init --depth 1 src/external/sha-1 src/external/sha-2
cargo build --release      # → target/release/osu-skin-renamer(.exe)
cargo test --release
```

Needs CMake and a C++ compiler (MSVC on Windows). On Linux also the
`webkit2gtk-4.1` and `gtk3` dev packages.

realm-core **v20.1.2** is the core inside osu!lazer's Realm .NET 20.1.0. If
lazer upgrades Realm, bump this tag (here and in the release workflow).

Pushing a tag like `v2.0.0` runs a GitHub Actions workflow that builds the
Windows exe and the Linux binary and attaches them to the release.

## Safety

- Built-in skins (Argon, Triangles, Classic, …) are never touched.
- Format upgrades are disabled, so the database can never be upgraded to a
  format lazer can no longer read. If lazer uses a newer format than this
  tool's realm-core, the tool stops with a clear error and can't break anything.
- To restore a backup: close osu!, then copy the newest file from the
  backups folder back to `client.realm` (see *Where things live*).
