const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("skins", {
  list: () => ipcRenderer.invoke("skins:list"),
  save: (renames) => ipcRenderer.invoke("skins:save", renames),
  contextMenu: () => ipcRenderer.invoke("skins:menu"),
  delete: (id, name) => ipcRenderer.invoke("skins:delete", id, name),
});
