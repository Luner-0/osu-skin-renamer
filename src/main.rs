// No console window for the GUI; CLI mode re-attaches to the parent console.
#![windows_subsystem = "windows"]

mod gui;
mod realm;

use realm::{Db, Edit};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

const KEEP_BACKUPS: usize = 2;

fn home() -> PathBuf {
    PathBuf::from(env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")).unwrap_or_default())
}

/// lazer's data root: %APPDATA% on Windows, $XDG_DATA_HOME (~/.local/share) on Linux.
fn data_dir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(env::var_os("APPDATA").unwrap_or_default())
    } else {
        env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"))
    }
}

/// Same folder Electron used for userData, so config.json and backups carry over.
pub fn config_dir() -> PathBuf {
    let base = if cfg!(windows) {
        data_dir()
    } else {
        env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config"))
    };
    base.join("osu-skin-renamer")
}

/// Finds client.realm: honours lazer's "change folder location" (storage.ini → FullPath)
/// and Flatpak lazer, else the default path.
fn default_db() -> PathBuf {
    let base = data_dir().join("osu");
    let mut candidates = Vec::new();
    if let Ok(ini) = fs::read_to_string(base.join("storage.ini")) {
        for line in ini.lines() {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "FullPath" {
                    candidates.push(PathBuf::from(v.trim()));
                }
            }
        }
    }
    candidates.push(base.clone());
    if cfg!(target_os = "linux") {
        candidates.push(home().join(".var/app/sh.ppy.osu/data/osu"));
    }
    let found = candidates.into_iter().find(|d| d.join("client.realm").exists());
    found.unwrap_or(base).join("client.realm")
}

/// Accepts the client.realm file or its folder; None = auto-detect.
pub fn resolve_db(p: Option<&str>) -> PathBuf {
    match p {
        None => default_db(),
        Some(p) if Path::new(p).is_dir() => Path::new(p).join("client.realm"),
        Some(p) => PathBuf::from(p),
    }
}

fn osu_is_running() -> bool {
    use std::process::Command;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq osu!.exe", "/FO", "CSV", "/NH"])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("osu!.exe"))
            .unwrap_or(false) // if tasklist fails, don't block — the backup still protects us
    }
    #[cfg(not(windows))]
    {
        Command::new("pgrep").args(["-x", "osu!"]).output().map(|o| o.status.success()).unwrap_or(false)
    }
}

/// Opens the db, refusing while osu! runs: lazer locks client.realm and
/// opening it then blocks forever. (Copies under other names skip the check.)
pub fn open_db(path: &Path) -> Result<Db, String> {
    if !path.exists() {
        return Err(format!("osu! database not found: {}", path.display()));
    }
    if path.file_name().is_some_and(|n| n == "client.realm") && osu_is_running() {
        return Err("osu! is running — close it first, then try again.".into());
    }
    Db::open(path)
}

// ---- backups: client.realm.2026-11-20T12-34-56-789Z.bak ----

fn days_to_civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (m <= 2) as i64, m, d)
}

fn civil_to_days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

fn stamp(ms: i64) -> String {
    let (y, mo, d) = days_to_civil(ms.div_euclid(86_400_000));
    let t = ms.rem_euclid(86_400_000);
    format!("{y:04}-{mo:02}-{d:02}T{:02}-{:02}-{:02}-{:03}Z", t / 3_600_000, t / 60_000 % 60, t / 1000 % 60, t % 1000)
}

fn parse_stamp(s: &str) -> Option<i64> {
    let n: Vec<i64> = s.strip_suffix('Z')?.split(['-', 'T']).map(|x| x.parse().ok()).collect::<Option<_>>()?;
    let [y, mo, d, h, mi, sec, ms] = n[..] else { return None };
    Some(civil_to_days(y, mo, d) * 86_400_000 + ((h * 60 + mi) * 60 + sec) * 1000 + ms)
}

fn backup_stamps(dir: &Path) -> Vec<(i64, PathBuf)> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let t = parse_stamp(name.strip_prefix("client.realm.")?.strip_suffix(".bak")?)?;
            Some((t, e.path()))
        })
        .collect();
    v.sort();
    v
}

/// One backup per 10-minute window (so a renaming session doesn't copy the
/// whole db each save), keeping only the newest few.
pub fn make_backup(db: &Path) -> Result<PathBuf, String> {
    let dir = config_dir().join("backups");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
    if let Some((t, p)) = backup_stamps(&dir).pop() {
        if now - t < 10 * 60 * 1000 {
            return Ok(p);
        }
    }
    let dest = dir.join(format!("client.realm.{}.bak", stamp(now)));
    fs::copy(db, &dest).map_err(|e| format!("Backup failed: {e}"))?;
    let all = backup_stamps(&dir);
    for (_, p) in &all[..all.len().saturating_sub(KEEP_BACKUPS)] {
        let _ = fs::remove_file(p);
    }
    Ok(dest)
}

/// Backup, then apply edits in one transaction.
pub fn write_edits(db_path: &Path, edits: &[(String, Edit)]) -> Result<(Vec<(String, String)>, PathBuf), String> {
    let db = open_db(db_path)?; // checks osu! isn't running before we copy
    let backup = make_backup(db_path)?;
    Ok((db.apply(edits)?, backup))
}

// ---- CLI ----

const USAGE: &str = "osu-skin-renamer — run without arguments for the GUI. Commands:
  list                                    show all skins
  set --id <guid> --name <text>           rename a single skin
  delete --id <guid>                      delete a single skin (soft delete)
  export [file]                           write GUID<TAB>name list (default names.tsv)
  apply [file] [--dry-run]                apply edited names.tsv
  rename --match <regex> --replace <txt>  regex bulk rename [--dry-run]
  (--db <path> to target a different client.realm or osu! folder)";

fn cli(argv: Vec<String>) -> Result<(), String> {
    let mut pos = Vec::new();
    let (mut db, mut id, mut name, mut re, mut rep, mut dry) = (None, None, None, None, None, false);
    let mut it = argv.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dry-run" => dry = true,
            "--db" => db = it.next(),
            "--id" => id = it.next(),
            "--name" => name = it.next(),
            "--match" => re = it.next(),
            "--replace" => rep = it.next(),
            _ => pos.push(a),
        }
    }
    let db_path = resolve_db(db.as_deref());
    let file = || pos.get(1).cloned().unwrap_or_else(|| "names.tsv".into());

    let edits: Vec<(String, Edit)> = match pos.first().map(String::as_str) {
        Some("list") => {
            let skins = open_db(&db_path)?.skins()?;
            println!("{} skins:\n", skins.len());
            for s in skins {
                let by = if s.creator.is_empty() { String::new() } else { format!("  (by {})", s.creator) };
                println!("  {}  {}{by}", s.id, s.name);
            }
            return Ok(());
        }
        Some("export") => {
            let skins = open_db(&db_path)?.skins()?;
            let mut out = String::from("# Edit the names in the second column, then run:  osu-skin-renamer apply\n");
            for s in &skins {
                out += &format!("{}\t{}\n", s.id, s.name);
            }
            fs::write(file(), out).map_err(|e| e.to_string())?;
            println!("Exported {} skins to {}", skins.len(), file());
            return Ok(());
        }
        Some("set") => {
            let (Some(id), Some(name)) = (id, name) else { return Err("Usage: set --id <guid> --name <text>".into()) };
            vec![(id, Edit::Rename(name))]
        }
        Some("delete") => {
            let Some(id) = id else { return Err("Usage: delete --id <guid>".into()) };
            vec![(id, Edit::Delete)]
        }
        Some("apply") => {
            let text = fs::read_to_string(file()).map_err(|_| format!("File not found: {}  (run 'export' first)", file()))?;
            let wanted = parse_tsv(&text);
            let skins = open_db(&db_path)?.skins()?;
            plan(&skins, |s| wanted.iter().find(|(id, _)| id.eq_ignore_ascii_case(&s.id)).map(|(_, n)| n.clone()))
        }
        Some("rename") => {
            let (Some(re), Some(rep)) = (re, rep) else { return Err("Usage: rename --match <regex> --replace <text> [--dry-run]".into()) };
            let re = regex::Regex::new(&re).map_err(|e| e.to_string())?;
            let skins = open_db(&db_path)?.skins()?;
            plan(&skins, |s| re.is_match(&s.name).then(|| re.replace_all(&s.name, rep.as_str()).into_owned()))
        }
        _ => {
            println!("{USAGE}");
            return Ok(());
        }
    };

    if edits.is_empty() {
        println!("Nothing to change.");
        return Ok(());
    }
    if dry {
        for (id, e) in &edits {
            if let Edit::Rename(n) = e {
                println!("  {id}  ->  \"{n}\"");
            }
        }
        println!("\nDry run: {} change(s) NOT applied.", edits.len());
        return Ok(());
    }
    let (done, backup) = write_edits(&db_path, &edits)?;
    println!("Backup: {}", backup.display());
    for ((_, from), (_, e)) in done.iter().zip(&edits) {
        match e {
            Edit::Rename(to) => println!("  \"{from}\"  ->  \"{to}\""),
            Edit::Delete => println!("  Deleted \"{from}\" (osu! removes it on next start)"),
        }
    }
    if done.len() != edits.len() {
        println!("{} id(s) not found.", edits.len() - done.len());
    }
    Ok(())
}

/// GUID <TAB> name lines; '#' comments and blank lines skipped.
fn parse_tsv(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('\t'))
        .map(|(id, n)| (id.trim().to_string(), n.trim().to_string()))
        .filter(|(id, n)| !id.is_empty() && !n.is_empty())
        .collect()
}

fn plan(skins: &[realm::Skin], new_name: impl Fn(&realm::Skin) -> Option<String>) -> Vec<(String, Edit)> {
    skins
        .iter()
        .filter_map(|s| new_name(s).filter(|n| !n.is_empty() && *n != s.name).map(|n| (s.id.clone(), Edit::Rename(n))))
        .collect()
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return gui::run();
    }
    #[cfg(windows)]
    {
        extern "system" {
            fn AttachConsole(pid: u32) -> i32;
        }
        unsafe { AttachConsole(u32::MAX) }; // ATTACH_PARENT_PROCESS
    }
    if let Err(e) = cli(args) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

#[test]
fn backup_stamp_roundtrip() {
    for ms in [0, 1_763_640_896_789, 951_782_400_000 /* 2000-02-29 */] {
        assert_eq!(parse_stamp(&stamp(ms)), Some(ms));
    }
    assert_eq!(stamp(1_763_640_896_789), "2025-11-20T12-14-56-789Z");
    assert_eq!(parse_tsv("# c\nab\tNew \n\nx\t \nnotab"), vec![("ab".into(), "New".into())]);
}
