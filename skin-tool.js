#!/usr/bin/env node
/*
 * osu-skin-renamer — list & bulk-rename osu!lazer skins
 *
 * Works directly on lazer's client.realm database via the Realm SDK.
 * Safety rails:
 *   - refuses to write while osu! is running
 *   - creates a timestamped backup of client.realm before every write
 *   - opens with disableFormatUpgrade so it can never silently upgrade
 *     the database to a format lazer can't read anymore
 *
 * Usage:
 *   node skin-tool.js list                      show all skins
 *   node skin-tool.js export [file]             write names to names.tsv (GUID <TAB> name)
 *   node skin-tool.js apply  [file] [--dry-run] read edited names.tsv and rename in bulk
 *   node skin-tool.js rename --match <regex> --replace <text> [--dry-run]
 *
 *   --db <path>   use a different client.realm (default: %APPDATA%\osu\client.realm)
 */

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const Realm = require("realm");

const DEFAULT_DB = path.join(process.env.APPDATA, "osu", "client.realm");
// Overridable because in a packaged portable exe __dirname lives in an
// unpack-to-temp directory that is wiped between runs.
const BACKUP_DIR = process.env.SKIN_TOOL_BACKUP_DIR || path.join(__dirname, "backups");
const KEEP_BACKUPS = 2;

function parseArgs(argv) {
  const args = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--dry-run") args.dryRun = true;
    else if (a === "--json") args.json = true;
    else if (a === "--db") args.db = argv[++i];
    else if (a === "--id") args.id = argv[++i];
    else if (a === "--name") args.name = argv[++i];
    else if (a === "--match") args.match = argv[++i];
    else if (a === "--replace") args.replace = argv[++i];
    else args._.push(a);
  }
  return args;
}

function osuIsRunning() {
  try {
    const out = execSync('tasklist /FI "IMAGENAME eq osu!.exe" /FO CSV /NH', { encoding: "utf8" });
    return /osu!\.exe/i.test(out);
  } catch {
    return false; // if tasklist fails, don't block — backup still protects us
  }
}

function makeBackup(dbPath) {
  fs.mkdirSync(BACKUP_DIR, { recursive: true });
  // Renaming many skins in a row would otherwise copy the whole DB once per
  // rename; a backup per 10-minute window still preserves the pre-session state.
  // Age comes from the timestamp in the filename — file mtime is useless here
  // because copyFileSync on Windows preserves the source's mtime.
  let newest = null;
  let newestTime = 0;
  for (const f of fs.readdirSync(BACKUP_DIR)) {
    const m = f.match(/^client\.realm\.(\d{4}-\d{2}-\d{2})T(\d{2})-(\d{2})-(\d{2})-(\d{3})Z\.bak$/);
    if (!m) continue;
    const t = Date.parse(`${m[1]}T${m[2]}:${m[3]}:${m[4]}.${m[5]}Z`);
    if (t > newestTime) {
      newestTime = t;
      newest = path.join(BACKUP_DIR, f);
    }
  }
  if (newest && Date.now() - newestTime < 10 * 60 * 1000) return newest;
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const dest = path.join(BACKUP_DIR, `client.realm.${stamp}.bak`);
  fs.copyFileSync(dbPath, dest);
  // Keep only the newest few — each backup is a full copy of client.realm.
  // ISO timestamps in the names sort chronologically.
  const all = fs.readdirSync(BACKUP_DIR).filter((f) => /^client\.realm\..+\.bak$/.test(f)).sort();
  for (const f of all.slice(0, -KEEP_BACKUPS)) fs.rmSync(path.join(BACKUP_DIR, f), { force: true });
  return dest;
}

function openRealm(dbPath) {
  if (!fs.existsSync(dbPath)) {
    console.error(`Database not found: ${dbPath}`);
    process.exit(1);
  }
  // lazer holds an exclusive lock on client.realm while running; opening it
  // then blocks forever instead of failing, so bail out up front.
  if (path.resolve(dbPath) === path.resolve(DEFAULT_DB) && osuIsRunning()) {
    console.error("osu! is running — close it first, then try again.");
    process.exit(1);
  }
  try {
    // No schema given -> dynamic mode, uses whatever schema the file has.
    return new Realm({ path: dbPath, disableFormatUpgrade: true });
  } catch (e) {
    if (/format|version/i.test(String(e))) {
      console.error(
        "Cannot open this client.realm: its file format is newer than this tool's Realm SDK.\n" +
        "Renaming via this tool is not possible until the 'realm' npm package catches up.\n" +
        `(${e.message})`
      );
    } else {
      console.error(`Failed to open realm: ${e.message}`);
    }
    process.exit(1);
  }
}

function getSkins(realm) {
  // Protected = built-in skins (Argon, Triangles, Classic, ...) — never touch those.
  return realm
    .objects("Skin")
    .filtered("Protected == false AND DeletePending == false")
    .sorted("Name");
}

function cmdList(args) {
  const realm = openRealm(args.db || DEFAULT_DB);
  const skins = getSkins(realm);
  if (args.json) {
    const out = skins.map((s) => ({ id: String(s.ID), name: s.Name, creator: s.Creator || "" }));
    console.log(JSON.stringify(out));
  } else {
    console.log(`${skins.length} skins:\n`);
    for (const s of skins) {
      const creator = s.Creator ? `  (by ${s.Creator})` : "";
      console.log(`  ${s.ID}  ${s.Name}${creator}`);
    }
  }
  realm.close();
}

function cmdSet(args) {
  if (!args.id || !args.name) {
    console.error("Usage: node skin-tool.js set --id <guid> --name <text>");
    process.exit(1);
  }
  const dbPath = args.db || DEFAULT_DB;
  if (osuIsRunning()) {
    console.error("osu! is running — close it first, then try again.");
    process.exit(1);
  }
  const backup = makeBackup(dbPath);
  const realm = openRealm(dbPath);
  const skin = getSkins(realm).find((s) => String(s.ID) === args.id.toLowerCase());
  if (!skin) {
    realm.close();
    console.error(`No skin with ID ${args.id}`);
    process.exit(1);
  }
  const from = skin.Name;
  realm.write(() => {
    skin.Name = args.name;
  });
  realm.close();
  if (args.json) {
    console.log(JSON.stringify({ ok: true, id: args.id, from, to: args.name, backup }));
  } else {
    console.log(`Backup: ${backup}`);
    console.log(`"${from}"  ->  "${args.name}"`);
  }
}

function cmdDelete(args) {
  if (!args.id) {
    console.error("Usage: node skin-tool.js delete --id <guid>");
    process.exit(1);
  }
  const dbPath = args.db || DEFAULT_DB;
  if (osuIsRunning()) {
    console.error("osu! is running — close it first, then try again.");
    process.exit(1);
  }
  const backup = makeBackup(dbPath);
  const realm = openRealm(dbPath);
  const skin = getSkins(realm).find((s) => String(s.ID) === args.id.toLowerCase());
  if (!skin) {
    realm.close();
    console.error(`No skin with ID ${args.id}`);
    process.exit(1);
  }
  const name = skin.Name;
  // Same soft delete lazer uses: mark DeletePending, lazer purges it on next start.
  realm.write(() => {
    skin.DeletePending = true;
  });
  realm.close();
  if (args.json) {
    console.log(JSON.stringify({ ok: true, id: args.id, name, backup }));
  } else {
    console.log(`Backup: ${backup}`);
    console.log(`Deleted "${name}" (marked DeletePending; osu! removes it on next start).`);
  }
}

function cmdExport(args) {
  const outFile = args._[1] || path.join(__dirname, "names.tsv");
  const realm = openRealm(args.db || DEFAULT_DB);
  const skins = getSkins(realm);
  const lines = ["# Edit the names in the second column, then run:  node skin-tool.js apply"];
  for (const s of skins) lines.push(`${s.ID}\t${s.Name}`);
  fs.writeFileSync(outFile, lines.join("\n") + "\n", "utf8");
  console.log(`Exported ${skins.length} skins to ${outFile}`);
  realm.close();
}

function loadRenamesFromTsv(file) {
  const renames = new Map();
  for (const raw of fs.readFileSync(file, "utf8").split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const tab = line.indexOf("\t");
    if (tab === -1) continue;
    const id = line.slice(0, tab).trim();
    const name = line.slice(tab + 1).trim();
    if (id && name) renames.set(id, name);
  }
  return renames;
}

function applyRenames(args, planFn) {
  const dbPath = args.db || DEFAULT_DB;
  if (osuIsRunning()) {
    console.error("osu! is running — close it first, then try again.");
    process.exit(1);
  }
  const realm = openRealm(dbPath);
  const skins = getSkins(realm);
  const plan = [];
  for (const s of skins) {
    const newName = planFn(s);
    if (newName != null && newName !== "" && newName !== s.Name) {
      plan.push({ id: String(s.ID), from: s.Name, to: newName });
    }
  }
  realm.close();
  if (plan.length === 0) {
    console.log(args.json ? JSON.stringify({ ok: true, renamed: 0 }) : "Nothing to rename.");
    return;
  }
  if (!args.json) for (const p of plan) console.log(`  "${p.from}"  ->  "${p.to}"`);
  if (args.dryRun) {
    console.log(`\nDry run: ${plan.length} rename(s) NOT applied.`);
    return;
  }
  const backup = makeBackup(dbPath);
  if (!args.json) console.log(`Backup: ${backup}`);
  const realm2 = openRealm(dbPath);
  const byId = new Map();
  for (const s of getSkins(realm2)) byId.set(String(s.ID), s);
  realm2.write(() => {
    for (const p of plan) {
      const s = byId.get(p.id);
      if (s) s.Name = p.to;
    }
  });
  realm2.close();
  console.log(args.json
    ? JSON.stringify({ ok: true, renamed: plan.length, backup })
    : `\nDone: ${plan.length} skin(s) renamed.`);
}

function cmdApply(args) {
  const file = args._[1] || path.join(__dirname, "names.tsv");
  if (!fs.existsSync(file)) {
    console.error(`File not found: ${file}  (run 'export' first)`);
    process.exit(1);
  }
  const renames = loadRenamesFromTsv(file);
  applyRenames(args, (s) => renames.get(String(s.ID)));
}

function cmdRename(args) {
  if (!args.match || args.replace == null) {
    console.error("Usage: node skin-tool.js rename --match <regex> --replace <text> [--dry-run]");
    process.exit(1);
  }
  applyRenames(args, (s) => {
    const re = new RegExp(args.match, "g");
    return re.test(s.Name) ? s.Name.replace(new RegExp(args.match, "g"), args.replace) : null;
  });
}

const args = parseArgs(process.argv.slice(2));
const cmd = args._[0];
switch (cmd) {
  case "list":   cmdList(args); break;
  case "set":    cmdSet(args); break;
  case "delete": cmdDelete(args); break;
  case "export": cmdExport(args); break;
  case "apply":  cmdApply(args); break;
  case "rename": cmdRename(args); break;
  default:
    console.log(
      "osu-skin-renamer — commands:\n" +
      "  list [--json]                           show all skins\n" +
      "  set --id <guid> --name <text>           rename a single skin\n" +
      "  delete --id <guid>                      delete a single skin (soft delete)\n" +
      "  export [file]                           write GUID<TAB>name list to names.tsv\n" +
      "  apply [file] [--dry-run]                apply edited names.tsv\n" +
      "  rename --match <regex> --replace <txt>  regex bulk rename [--dry-run]\n" +
      "  (--db <path> to target a different client.realm)"
    );
}
// Realm keeps the event loop alive even after close(); all commands are
// synchronous, so force a clean exit here.
process.exit(0);
