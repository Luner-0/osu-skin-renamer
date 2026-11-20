//! Native window + system webview (WebView2 / WebKitGTK) showing index.html.
//! The page calls `window.ipc.postMessage({id, cmd, args})`; replies go back
//! through `__reply(id, ok, value)`.

use crate::realm::Edit;
use crate::{config_dir, open_db, resolve_db, write_edits};
use serde_json::{json, Value};
use std::borrow::Cow;
use std::fs;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Window, WindowBuilder};
use wry::http::Response;
use wry::{WebContext, WebViewBuilder};

const INDEX: &str = include_str!("index.html");

fn load_config() -> Value {
    fs::read_to_string(config_dir().join("config.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn osu_folder() -> Option<String> {
    load_config()["osuFolder"].as_str().map(String::from)
}

fn handle(cmd: &str, args: &Value, window: &Window) -> Result<Value, String> {
    let db = || resolve_db(osu_folder().as_deref());
    match cmd {
        "list" => {
            let skins = open_db(&db())?.skins()?;
            Ok(skins.iter().map(|s| json!({"id": s.id, "name": s.name, "creator": s.creator})).collect())
        }
        "getFolder" => Ok(json!(osu_folder())),
        "chooseFolder" => {
            let mut cfg = load_config();
            let mut dlg = rfd::FileDialog::new()
                .set_title("Choose your osu! data folder (the one containing client.realm)")
                .set_parent(window);
            if let Some(f) = osu_folder() {
                dlg = dlg.set_directory(f);
            }
            let Some(dir) = dlg.pick_folder() else { return Ok(Value::Null) };
            if !dir.join("client.realm").exists() {
                return Err(format!(
                    "No client.realm in \"{}\". Pick the osu! data folder: the one with client.realm, files/ and storage.ini in it.",
                    dir.display()
                ));
            }
            cfg["osuFolder"] = json!(dir.to_string_lossy());
            fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
            fs::write(config_dir().join("config.json"), serde_json::to_string_pretty(&cfg).unwrap())
                .map_err(|e| e.to_string())?;
            Ok(cfg["osuFolder"].clone())
        }
        // One transaction + one backup for all pending renames.
        "save" => {
            let edits: Vec<_> = args[0]
                .as_array()
                .ok_or("bad save request")?
                .iter()
                .filter_map(|r| Some((r["id"].as_str()?.to_string(), Edit::Rename(r["name"].as_str()?.to_string()))))
                .collect();
            let (done, backup) = write_edits(&db(), &edits)?;
            Ok(json!({"renamed": done.len(), "backup": backup}))
        }
        "delete" => {
            let id = args[0].as_str().ok_or("bad delete request")?.to_string();
            let (done, backup) = write_edits(&db(), &[(id.clone(), Edit::Delete)])?;
            let (_, name) = done.into_iter().next().ok_or(format!("No skin with ID {id}"))?;
            Ok(json!({"name": name, "backup": backup}))
        }
        _ => Err(format!("unknown command {cmd}")),
    }
}

pub fn run() {
    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("osu! skin renamer")
        .with_inner_size(LogicalSize::new(960.0, 720.0))
        .build(&event_loop)
        .expect("create window");

    // Keeps localStorage (tags) in our config dir instead of next to the exe.
    let mut ctx = WebContext::new(Some(config_dir().join("webview")));
    let builder = WebViewBuilder::new_with_web_context(&mut ctx)
        .with_custom_protocol("app".into(), |_, _| {
            Response::builder()
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Cow::Borrowed(INDEX.as_bytes()))
                .unwrap()
        })
        .with_url("app://localhost/")
        .with_background_color((0x16, 0x12, 0x1c, 0xff))
        .with_ipc_handler(move |req| {
            let _ = proxy.send_event(req.into_body());
        });
    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window).expect("create webview");
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder.build_gtk(window.default_vbox().unwrap()).expect("create webview")
    };

    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            // The page asks to confirm if there are unsaved renames, then sends "quit".
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                let _ = webview.evaluate_script("__tryClose()");
            }
            Event::UserEvent(msg) => {
                let req: Value = serde_json::from_str(&msg).unwrap_or_default();
                let cmd = req["cmd"].as_str().unwrap_or_default();
                if cmd == "quit" {
                    *flow = ControlFlow::Exit;
                    return;
                }
                let (ok, val) = match handle(cmd, &req["args"], &window) {
                    Ok(v) => (true, v),
                    Err(e) => (false, json!(e)),
                };
                let _ = webview.evaluate_script(&format!("__reply({},{ok},{val})", req["id"]));
            }
            _ => {}
        }
    });
}
