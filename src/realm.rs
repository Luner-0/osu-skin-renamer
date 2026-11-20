//! Minimal hand-written bindings to realm-core's C API (src/realm.h) — only
//! what's needed to read and edit lazer's `Skin` objects.

use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;

type Ptr = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct Value {
    data: [u64; 2], // union realm_value.values (16 bytes)
    ty: i32,        // realm_value_type_e
}
const TYPE_NULL: i32 = 0;
const TYPE_BOOL: i32 = 2;
const TYPE_STRING: i32 = 3;
const TYPE_UUID: i32 = 11;

#[repr(C)]
struct ClassInfo {
    name: *const c_char,
    primary_key: *const c_char,
    num_properties: usize,
    num_computed_properties: usize,
    key: u32,
    flags: i32,
}

#[repr(C)]
struct PropertyInfo {
    name: *const c_char,
    public_name: *const c_char,
    ty: i32,
    collection_type: i32,
    link_target: *const c_char,
    link_origin_property_name: *const c_char,
    key: i64,
    flags: i32,
}

#[repr(C)]
struct Error {
    error: i32,
    categories: i32,
    message: *const c_char,
    user_code_error: Ptr,
    path: *const c_char,
}

extern "C" {
    fn realm_get_last_error(err: *mut Error) -> bool;
    fn realm_clear_last_error() -> bool;
    fn realm_release(p: Ptr);
    fn realm_config_new() -> Ptr;
    fn realm_config_set_path(c: Ptr, path: *const c_char);
    fn realm_config_set_disable_format_upgrade(c: Ptr, v: bool);
    fn realm_config_set_scheduler(c: Ptr, s: Ptr);
    fn realm_scheduler_new(
        ud: Ptr,
        free: Option<extern "C" fn(Ptr)>,
        notify: extern "C" fn(Ptr, Ptr),
        is_on_thread: extern "C" fn(Ptr) -> bool,
        is_same_as: extern "C" fn(Ptr, Ptr) -> bool,
        can_deliver: extern "C" fn(Ptr) -> bool,
    ) -> Ptr;
    fn realm_open(c: Ptr) -> Ptr;
    fn realm_close(r: Ptr) -> bool;
    fn realm_begin_write(r: Ptr) -> bool;
    fn realm_commit(r: Ptr) -> bool;
    fn realm_rollback(r: Ptr) -> bool;
    fn realm_find_class(r: Ptr, name: *const c_char, found: *mut bool, out: *mut ClassInfo) -> bool;
    fn realm_find_property(r: Ptr, class: u32, name: *const c_char, found: *mut bool, out: *mut PropertyInfo) -> bool;
    fn realm_query_parse(r: Ptr, class: u32, q: *const c_char, nargs: usize, args: *const c_void) -> Ptr;
    fn realm_query_find_all(q: Ptr) -> Ptr;
    fn realm_results_count(res: Ptr, out: *mut usize) -> bool;
    fn realm_results_get_object(res: Ptr, i: usize) -> Ptr;
    fn realm_get_value(obj: Ptr, prop: i64, out: *mut Value) -> bool;
    fn realm_set_value(obj: Ptr, prop: i64, v: Value, is_default: bool) -> bool;
}

/// Owned realm handle, released on drop.
struct Handle(Ptr);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { realm_release(self.0) }
    }
}

fn last_error() -> String {
    unsafe {
        let mut e: Error = std::mem::zeroed();
        if !realm_get_last_error(&mut e) || e.message.is_null() {
            return "unknown realm error".into();
        }
        let msg = CStr::from_ptr(e.message).to_string_lossy().into_owned();
        realm_clear_last_error();
        msg
    }
}

fn check(ok: bool) -> Result<(), String> {
    if ok { Ok(()) } else { Err(last_error()) }
}

fn handle(p: Ptr) -> Result<Handle, String> {
    if p.is_null() { Err(last_error()) } else { Ok(Handle(p)) }
}

fn cstr(s: &str) -> CString {
    CString::new(s).expect("no NUL in literal")
}

pub struct Skin {
    pub id: String,
    pub name: String,
    pub creator: String,
    /// Skin class from InstantiationInfo, e.g. "LegacySkin".
    pub ty: String,
}

pub enum Edit {
    /// Sets each field that is Some.
    Update { name: Option<String>, creator: Option<String> },
    Delete,
}

pub struct Db {
    realm: Handle,
    class: u32,
    id: i64,
    name: i64,
    creator: i64,
    instantiation: i64,
    delete_pending: i64,
}

impl Db {
    pub fn open(path: &Path) -> Result<Db, String> {
        let p = CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        unsafe {
            let cfg = Handle(realm_config_new());
            realm_config_set_path(cfg.0, p.as_ptr());
            // Never upgrade the file to a format lazer can't read.
            realm_config_set_disable_format_upgrade(cfg.0, true);
            // Core has no built-in scheduler on Windows. We use the realm
            // synchronously on one thread and never want notifications.
            extern "C" fn notify(_: Ptr, _: Ptr) {}
            extern "C" fn yes(_: Ptr) -> bool { true }
            extern "C" fn same(_: Ptr, _: Ptr) -> bool { true }
            extern "C" fn no(_: Ptr) -> bool { false }
            let sched = Handle(realm_scheduler_new(std::ptr::null_mut(), None, notify, yes, same, no));
            realm_config_set_scheduler(cfg.0, sched.0);
            // No schema set -> opens with whatever schema the file has.
            let realm = handle(realm_open(cfg.0)).map_err(|e| {
                if e.to_lowercase().contains("format") || e.to_lowercase().contains("version") {
                    format!("Cannot open this client.realm: its file format is newer than this tool's Realm.\n({e})")
                } else {
                    format!("Failed to open realm: {e}")
                }
            })?;
            let mut found = false;
            let mut ci: ClassInfo = std::mem::zeroed();
            check(realm_find_class(realm.0, cstr("Skin").as_ptr(), &mut found, &mut ci))?;
            if !found {
                return Err("No Skin table in this database — is it an osu!lazer client.realm?".into());
            }
            let prop = |name: &str| -> Result<i64, String> {
                let mut found = false;
                let mut pi: PropertyInfo = std::mem::zeroed();
                check(realm_find_property(realm.0, ci.key, cstr(name).as_ptr(), &mut found, &mut pi))?;
                if found { Ok(pi.key) } else { Err(format!("Skin has no {name} property")) }
            };
            Ok(Db {
                class: ci.key,
                id: prop("ID")?,
                name: prop("Name")?,
                creator: prop("Creator")?,
                instantiation: prop("InstantiationInfo")?,
                delete_pending: prop("DeletePending")?,
                realm,
            })
        }
    }

    /// User skins (Protected = built-ins like Argon/Triangles, never touched), sorted by name.
    fn objects(&self) -> Result<Vec<Handle>, String> {
        let q = cstr("Protected == false AND DeletePending == false SORT(Name ASC)");
        unsafe {
            let query = handle(realm_query_parse(self.realm.0, self.class, q.as_ptr(), 0, std::ptr::null()))?;
            let res = handle(realm_query_find_all(query.0))?;
            let mut n = 0;
            check(realm_results_count(res.0, &mut n))?;
            (0..n).map(|i| handle(realm_results_get_object(res.0, i))).collect()
        }
    }

    fn get(&self, obj: &Handle, prop: i64) -> Result<Value, String> {
        let mut v = Value { data: [0; 2], ty: TYPE_NULL };
        check(unsafe { realm_get_value(obj.0, prop, &mut v) })?;
        Ok(v)
    }

    fn get_string(&self, obj: &Handle, prop: i64) -> Result<String, String> {
        let v = self.get(obj, prop)?;
        if v.ty != TYPE_STRING {
            return Ok(String::new()); // null
        }
        let bytes = unsafe { std::slice::from_raw_parts(v.data[0] as *const u8, v.data[1] as usize) };
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    fn get_id(&self, obj: &Handle) -> Result<String, String> {
        let v = self.get(obj, self.id)?;
        if v.ty != TYPE_UUID {
            return Err("Skin.ID is not a UUID".into());
        }
        Ok(uuid_string(&unsafe { std::mem::transmute::<[u64; 2], [u8; 16]>(v.data) }))
    }

    pub fn skins(&self) -> Result<Vec<Skin>, String> {
        self.objects()?
            .iter()
            .map(|o| {
                Ok(Skin {
                    id: self.get_id(o)?,
                    name: self.get_string(o, self.name)?,
                    creator: self.get_string(o, self.creator)?,
                    ty: skin_type(&self.get_string(o, self.instantiation)?),
                })
            })
            .collect()
    }

    /// Applies all edits (keyed by skin id) in one transaction.
    /// Returns (id, old name) for each skin that was changed.
    pub fn apply(&self, edits: &[(String, Edit)]) -> Result<Vec<(String, String)>, String> {
        let objs = self.objects()?;
        let mut done = Vec::new();
        check(unsafe { realm_begin_write(self.realm.0) })?;
        let result = (|| {
            for o in &objs {
                let id = self.get_id(o)?;
                let Some((_, edit)) = edits.iter().find(|(e, _)| e.eq_ignore_ascii_case(&id)) else { continue };
                let old = self.get_string(o, self.name)?;
                let set = |prop, s: &String| {
                    let v = Value { data: [s.as_ptr() as u64, s.len() as u64], ty: TYPE_STRING };
                    check(unsafe { realm_set_value(o.0, prop, v, false) })
                };
                match edit {
                    Edit::Update { name, creator } => {
                        if let Some(n) = name {
                            set(self.name, n)?;
                        }
                        if let Some(c) = creator {
                            set(self.creator, c)?;
                        }
                    }
                    // Same soft delete lazer uses: it purges DeletePending skins on next start.
                    Edit::Delete => {
                        let v = Value { data: [1, 0], ty: TYPE_BOOL };
                        check(unsafe { realm_set_value(o.0, self.delete_pending, v, false) })?;
                    }
                }
                done.push((id, old));
            }
            check(unsafe { realm_commit(self.realm.0) })
        })();
        if let Err(e) = result {
            unsafe { realm_rollback(self.realm.0) };
            return Err(e);
        }
        Ok(done)
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        unsafe { realm_close(self.realm.0) };
    }
}

/// Canonical 8-4-4-4-12 lowercase hex, bytes in storage order (same as Realm JS's UUID.toString()).
fn uuid_string(b: &[u8; 16]) -> String {
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// "osu.Game.Skinning.LegacySkin, osu.Game" -> "LegacySkin"
fn skin_type(info: &str) -> String {
    let class = info.split(',').next().unwrap_or_default();
    class.rsplit('.').next().unwrap_or_default().trim().to_string()
}

#[test]
fn uuid_format() {
    assert_eq!(skin_type("osu.Game.Skinning.LegacySkin, osu.Game"), "LegacySkin");
    assert_eq!(skin_type(""), "");
    let b: [u8; 16] = std::array::from_fn(|i| i as u8);
    assert_eq!(uuid_string(&b), "00010203-0405-0607-0809-0a0b0c0d0e0f");
}
