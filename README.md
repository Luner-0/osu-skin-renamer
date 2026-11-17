# osu-skin-renamer

List, rename and tag osu!**lazer** skins, directly in the `client.realm`
database where lazer stores skin names.

**osu! must be closed while renaming.** A backup of `client.realm` is made
before every write.

## Download

Grab the latest build from [Releases](https://github.com/Luner-0/osu-skin-renamer/releases):

| File | Platform |
|---|---|
| `osu-skin-renamer-Setup-<version>.exe` | Windows installer. Run once, then start the app from the desktop / Start-menu shortcut. Windows SmartScreen may warn because the installer is unsigned (*More info → Run anyway*). |
| `osu-skin-renamer-<version>.pacman` | Arch Linux: `sudo pacman -U osu-skin-renamer-*.pacman` |
| `osu-skin-renamer-<version>.AppImage` | Any Linux distro: `chmod +x osu-skin-renamer-*.AppImage && ./osu-skin-renamer-*.AppImage` |

## Using the app

The window lists all your skins. Renaming works like Windows Explorer:
double-click a name (or select it and press **F2**), then **Enter** to accept
or **Esc** to cancel. **Right-click** opens a menu with *Rename* and *Delete*
(also the **Del** key). Deleting asks first and only marks the skin as
deleted; osu! removes it for good on its next start. The box at the top
filters the list.

- **Save / undo:** renames are only written to osu! when you press **Save**
  (**Ctrl+S**), all at once with a single backup. Unsaved skins are shown
  in pink. **Undo** (**Ctrl+Z**) and **Redo** (**Ctrl+Y**) step through
  your changes.
- **Tags:** add prefix and suffix tags (e.g. `[NM]`) in the panel on the
  right. Drag a tag onto a skin to add it; click a tag to toggle it on the
  selected skin.
- **Multi-select:** **Alt+click** to select several skins; tags then apply
  to all of them.

### Where things live

| | Windows | Linux |
|---|---|---|
| Skin database | `%APPDATA%\osu\client.realm` | `~/.local/share/osu/client.realm` (or `$XDG_DATA_HOME/osu`) |
| Backups (installed app) | `%APPDATA%\osu-skin-renamer\backups` | `~/.config/osu-skin-renamer/backups` |

Only the 2 newest backups are kept. When running from source, backups go to
`backups/` in the project folder.

The app finds osu! automatically, including a custom location set in
lazer's settings (*Change folder location*, read from `storage.ini`) and
Flatpak lazer (`~/.var/app/sh.ppy.osu/data/osu`). If it still can't find
it, click **📁 osu! folder** and pick the folder that contains
`client.realm`; the app remembers it.

## Command line

```
npm install
node skin-tool.js list
```
Shows all skins with their IDs. Every command accepts `--db <path>` (the
`client.realm` file or the folder containing it) to use a different database.

### Rename or delete a single skin

```
node skin-tool.js set --id <guid> --name "New name"
node skin-tool.js delete --id <guid>
```

### Bulk rename from a list (recommended)

```
node skin-tool.js export
```
Writes `names.tsv` (ID → name). Edit the names in the second column
(tab-separated); lines you don't want to change can be deleted or left as
they are. Then:

```
node skin-tool.js apply --dry-run   # preview
node skin-tool.js apply             # apply
```

### Bulk rename with a regex

```
node skin-tool.js rename --match "^Aristia" --replace "ARST" --dry-run
node skin-tool.js rename --match "\s+$" --replace ""        # strip trailing spaces
```

## Building from source

```
npm install
npm start              # run the app
npm run dist           # Windows installer → dist/
npm run dist:linux     # Arch .pacman + AppImage → dist/ (build on Linux)
```

Linux packages must be built on Linux, because Realm downloads its native
binary for the OS it's installed on. Pushing a tag like `v1.2.0` runs a
GitHub Actions workflow that builds the Windows installer and Linux packages
and attaches them to the release.

## Safety

- Built-in skins (Argon, Triangles, Classic, …) are never touched.
- `disableFormatUpgrade` stops the database from being upgraded to a format
  lazer can no longer read. If lazer uses a newer format than this tool's
  Realm SDK, the tool stops with a clear error and can't break anything.
- To restore a backup: close osu!, then copy the newest file from the
  backups folder back to `client.realm` (see *Where things live*).
