// Reimplements the old Electron preload.js "window.api" surface on top of
// Tauri's invoke/event API, so index.html and renderer.js need no changes.
(() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;

  window.api = {
    loadConfig: () => invoke("load_config"),
    saveSettings: (config) => invoke("save_settings", { config }),
    choosePath: (kind) => invoke("choose_path", { kind }),
    testFtp: (config) => invoke("test_ftp", { config }),
    startWatch: (config) => invoke("start_watch", { config }),
    stopWatch: () => invoke("stop_watch"),
    onLog: (cb) => listen("log", (event) => cb(event.payload)),
    onWatchState: (cb) => listen("watch-state", (event) => cb(event.payload))
  };
})();
