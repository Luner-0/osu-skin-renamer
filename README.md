# osu-skin-renamer

Liest und benennt osu!**lazer**-Skins um — direkt in der `client.realm`-Datenbank,
in der lazer die Skin-Namen speichert.

## Setup (einmalig)

```
npm install
```

## Benutzung

**Wichtig: osu! muss beim Umbenennen geschlossen sein.** Vor jedem Schreibvorgang
wird automatisch ein Backup von `client.realm` in `backups/` abgelegt.

```
node skin-tool.js list
```
Zeigt alle Skins mit ihrer ID.

### GUI (Electron)

```
npm start
```
Öffnet ein Fenster mit allen Skins. Umbenennen wie im Windows Explorer:
Doppelklick auf einen Namen (oder auswählen und **F2**), dann **Enter** zum
Übernehmen bzw. **Esc** zum Abbrechen. **Rechtsklick** öffnet ein Menü mit
*Rename* und *Delete* (auch per **Entf**-Taste); Löschen fragt vorher nach
und markiert den Skin nur als gelöscht — endgültig entfernt wird er beim
nächsten osu!-Start. Oben lässt sich die Liste filtern.

- **Speichern / Rückgängig:** Umbenennungen werden erst mit **Save**
  (**Strg+S**) in osu! geschrieben — alle auf einmal, mit einem Backup.
  Ungespeicherte Skins sind pink markiert. **Undo** (**Strg+Z**) und
  **Redo** (**Strg+Y**) gehen die Änderungen schrittweise durch.
- **Tags:** Rechts lassen sich Prefix- und Suffix-Tags anlegen (z. B.
  `[NM]`). Tag auf einen Skin ziehen fügt ihn hinzu; Klick auf einen Tag
  schaltet ihn beim ausgewählten Skin an/aus.
- **Mehrfachauswahl:** Mit **Alt+Klick** mehrere Skins auswählen — Tags
  gelten dann für alle.

Auch hier gilt: osu! muss geschlossen sein. Es werden nur die 2 neuesten
Backups behalten.

### Linux (Arch)

Unter *Releases* die `.pacman`-Datei herunterladen und installieren:

```
sudo pacman -U osu-skin-renamer-*.pacman
```

Alternativ das `.AppImage` (läuft auf jeder Distribution):
`chmod +x osu-skin-renamer-*.AppImage && ./osu-skin-renamer-*.AppImage`

Die Skin-Datenbank wird unter `~/.local/share/osu/client.realm` gesucht
(bzw. `$XDG_DATA_HOME/osu`). Wer lazer per Flatpak installiert hat, findet
sie unter `~/.var/app/sh.ppy.osu/data/osu/` — dann per CLI mit
`--db <pfad>` angeben.

Selbst bauen (auf Linux): `npm install && npm run dist:linux`

### Windows-Installer bauen

```
npm run dist
```
Erzeugt `dist/osu-skin-renamer Setup <version>.exe`. Fertige Installer gibt
es auch unter *Releases*.

### Einzelnen Skin per CLI umbenennen / löschen

```
node skin-tool.js set --id <guid> --name "Neuer Name"
node skin-tool.js delete --id <guid>
```

### Bulk-Rename über eine Liste (empfohlen)

```
node skin-tool.js export
```
Erzeugt `names.tsv` (ID → Name). Öffne die Datei, ändere die Namen in der
zweiten Spalte (Tab-getrennt); Zeilen, die du nicht ändern willst, kannst du
löschen oder stehen lassen. Dann:

```
node skin-tool.js apply --dry-run   # Vorschau
node skin-tool.js apply             # anwenden
```

### Bulk-Rename per Regex

```
node skin-tool.js rename --match "^Aristia" --replace "ARST" --dry-run
node skin-tool.js rename --match "\s+$" --replace ""        # trailing spaces entfernen
```

## Sicherheit

- Eingebaute Skins (Argon, Triangles, Classic …) werden nie angefasst.
- `disableFormatUpgrade` verhindert, dass die DB auf ein Format aktualisiert
  wird, das lazer nicht mehr lesen kann. Falls umgekehrt lazer ein neueres
  Format nutzt als das Realm-SDK dieses Tools, bricht das Tool mit einer
  klaren Fehlermeldung ab — es kann dann nichts kaputt machen.
- Backup wiederherstellen: osu! schließen, neueste Datei aus `backups/`
  zurück nach `%APPDATA%\osu\client.realm` kopieren.
