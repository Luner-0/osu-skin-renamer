const { app, BrowserWindow, ipcMain, Menu, dialog } = require("electron");
const path = require("path");
const { execFile } = require("child_process");

const TOOL = path.join(__dirname, "skin-tool.js");

// All database access goes through skin-tool.js in a child Node process.
// Instead of relying on a system Node (which a shared portable exe can't),
// the app re-runs its own binary in Node mode via ELECTRON_RUN_AS_NODE.
// Realm's native binding is N-API, so it loads fine under Electron's Node.
function runTool(args) {
  const env = { ...process.env, ELECTRON_RUN_AS_NODE: "1" };
  if (app.isPackaged) {
    env.SKIN_TOOL_BACKUP_DIR = path.join(app.getPath("userData"), "backups");
  }
  return new Promise((resolve, reject) => {
    execFile(
      process.execPath,
      [TOOL, ...args, "--json"],
      { cwd: __dirname, windowsHide: true, timeout: 30000, env },
      (err, stdout, stderr) => {
        if (err) return reject(new Error(stderr.trim() || err.message));
        try {
          resolve(JSON.parse(stdout));
        } catch {
          reject(new Error(`Unexpected tool output: ${stdout.trim()}`));
        }
      }
    );
  });
}

// Smoke-test hook: --screenshot=<path> captures the window and exits.
const shotArg = process.argv.find((a) => a.startsWith("--screenshot="));
if (shotArg) app.disableHardwareAcceleration();

app.whenReady().then(async () => {
  ipcMain.handle("skins:list", () => runTool(["list"]));
  // Batch save: one DB write + one backup for all pending renames,
  // via skin-tool's `apply` (GUID <TAB> name file).
  ipcMain.handle("skins:save", async (_e, renames) => {
    const fs = require("fs");
    const file = path.join(app.getPath("temp"), `skin-renames-${process.pid}.tsv`);
    fs.writeFileSync(file, renames.map((r) => `${r.id}\t${r.name}`).join("\n"), "utf8");
    try {
      return await runTool(["apply", file]);
    } finally {
      fs.rmSync(file, { force: true });
    }
  });

  // Right-click menu on a row. Resolves with the chosen action, or null
  // when the menu is dismissed (the popup callback fires before click
  // handlers, hence the setTimeout).
  ipcMain.handle("skins:menu", (e) => {
    const owner = BrowserWindow.fromWebContents(e.sender);
    return new Promise((resolve) => {
      const menu = Menu.buildFromTemplate([
        { label: "Rename", click: () => resolve("rename") },
        { type: "separator" },
        { label: "Delete", click: () => resolve("delete") },
      ]);
      menu.popup({ window: owner, callback: () => setTimeout(() => resolve(null), 50) });
    });
  });

  ipcMain.handle("skins:delete", async (e, id, name) => {
    const owner = BrowserWindow.fromWebContents(e.sender);
    const { response } = await dialog.showMessageBox(owner, {
      type: "warning",
      buttons: ["Delete", "Cancel"],
      defaultId: 1,
      cancelId: 1,
      message: `Delete skin "${name}"?`,
      detail:
        "The skin is marked for deletion and disappears when osu! next starts. " +
        "A backup of client.realm is created first.",
    });
    if (response !== 0) return { cancelled: true };
    return runTool(["delete", "--id", id]);
  });

  const win = new BrowserWindow({
    width: 960,
    height: 720,
    autoHideMenuBar: true,
    backgroundColor: "#16121c",
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      offscreen: Boolean(shotArg),
    },
  });
  // Alt is used for multi-select; don't let it pop up the hidden menu bar.
  win.webContents.on("before-input-event", (e, input) => {
    if (input.key === "Alt") e.preventDefault();
  });
  // Renderer blocks unload while renames are unsaved; ask before discarding.
  win.webContents.on("will-prevent-unload", (e) => {
    const choice = dialog.showMessageBoxSync(win, {
      type: "warning",
      buttons: ["Discard changes", "Cancel"],
      defaultId: 1,
      cancelId: 1,
      message: "You have unsaved renames.",
      detail: "Close anyway and lose them?",
    });
    if (choice === 0) e.preventDefault();
  });
  let lastFrame = null;
  if (shotArg) {
    win.webContents.on("console-message", (_e, _lvl, msg) =>
      console.log("smoke console:", msg)
    );
    win.webContents.on("paint", (_e, _dirty, image) => {
      lastFrame = image;
    });
    win.webContents.setFrameRate(10);
  }
  await win.loadFile("index.html");

  if (shotArg) {
    setTimeout(async () => {
      try {
        // Wait for the skin list to populate (or an error to land) before
        // capturing — the first child-process spawn can take a while.
        let rows = 0;
        for (let i = 0; i < 30; i++) {
          rows = await win.webContents.executeJavaScript(
            'document.querySelectorAll(".row").length'
          );
          const failed = await win.webContents.executeJavaScript(
            'document.getElementById("status").className === "error"'
          );
          if (rows > 0 || failed) break;
          await new Promise((r) => setTimeout(r, 500));
        }
        const status = await win.webContents.executeJavaScript(
          'document.getElementById("status").textContent'
        );
        const api = await win.webContents.executeJavaScript("typeof window.skins");
        console.log(`smoke: rows=${rows} api=${api} status="${status}"`);
        if (lastFrame) {
          require("fs").writeFileSync(shotArg.slice("--screenshot=".length), lastFrame.toPNG());
          console.log("smoke: screenshot written");
        } else {
          console.log("smoke: no frame painted");
        }
      } catch (e) {
        console.log("smoke error:", e.message);
      }
      app.quit();
    }, 2500);
  }
});

app.on("window-all-closed", () => app.quit());
