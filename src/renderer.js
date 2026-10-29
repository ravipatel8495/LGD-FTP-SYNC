const $ = (id) => document.getElementById(id);
const fields = ["protocol", "host", "port", "user", "password"];
const PRODUCT_TYPES = ["LGD", "NATURAL"];

const DEFAULT_PORTS = { ftp: "21", ftps: "21", "ftps-implicit": "990", sftp: "22" };

let watchRows = [{ type: "LGD", path: "" }];
let saveTimer = null;

function unusedType() {
  const used = new Set(watchRows.map((r) => r.type));
  return PRODUCT_TYPES.find((t) => !used.has(t));
}

function localPathsFromRows() {
  const localPaths = { LGD: "", NATURAL: "" };
  for (const row of watchRows) {
    if (row.path) localPaths[row.type] = row.path;
  }
  return localPaths;
}

function getConfig() {
  const localPaths = localPathsFromRows();
  const first = watchRows[0] || { type: "LGD", path: "" };
  const c = {
    productType: first.type,
    localPath: first.path || localPaths[first.type] || "",
    localPaths
  };
  for (const f of fields) c[f] = $(f).value.trim();
  return c;
}

function rowsFromConfig(c) {
  const lgd = (c.localPaths && c.localPaths.LGD) || "";
  const natural = (c.localPaths && c.localPaths.NATURAL) || "";
  const rows = [];
  if (lgd) rows.push({ type: "LGD", path: lgd });
  if (natural) rows.push({ type: "NATURAL", path: natural });
  if (rows.length === 0) {
    const type = c.productType === "NATURAL" ? "NATURAL" : "LGD";
    rows.push({ type, path: c.localPath || "" });
  }
  return rows;
}

function setConfig(c) {
  if (!c) {
    watchRows = [{ type: "LGD", path: "" }];
    renderWatchRows();
    return;
  }
  for (const f of fields) if (c[f] != null) $(f).value = c[f];
  watchRows = rowsFromConfig(c);
  renderWatchRows();
}

function setStatus(ok, message) {
  const el = $("status");
  el.className = ok ? "ok" : "error";
  el.textContent = message;
}

function addLog({ type, message, time, percent, key }) {
  const log = $("log");
  let line = null;
  if (key) {
    const esc = (window.CSS && CSS.escape) ? CSS.escape(String(key)) : String(key).replace(/"/g, '\\"');
    line = log.querySelector(`[data-key="${esc}"]`);
  }
  if (!line) {
    line = document.createElement("div");
    line.className = "logLine";
    if (key) line.dataset.key = String(key);
    const text = document.createElement("div");
    text.className = "logText";
    line.appendChild(text);
    log.appendChild(line);
  }
  line.className = `logLine ${type || "info"}`;
  const text = line.querySelector(".logText") || line;
  text.textContent = `[${time}] ${message}`;
  let bar = line.querySelector(".pctBar");
  const showBar = type === "progress" && percent != null && percent < 100;
  if (showBar) {
    if (!bar) {
      bar = document.createElement("div");
      bar.className = "pctBar";
      const fill = document.createElement("span");
      bar.appendChild(fill);
      line.appendChild(bar);
    }
    bar.classList.remove("done");
    bar.style.display = "block";
    const fill = bar.querySelector("span");
    if (fill) fill.style.width = `${Math.max(0, Math.min(99, percent))}%`;
  } else if (bar) {
    bar.classList.add("done");
    bar.style.display = "none";
  }
  log.scrollTop = log.scrollHeight;
}

async function layoutWindow() {
  const tw = window.__TAURI__ && window.__TAURI__.window;
  const WIDTH = 820;
  let maxH = 740;
  try {
    if (tw && tw.getCurrentWindow) {
      const win = tw.getCurrentWindow();
      const mon = await win.currentMonitor();
      if (mon && mon.size) {
        const scale = mon.scaleFactor || 1;
        maxH = Math.max(560, Math.floor(mon.size.height / scale) - 90);
      }
      if (tw.LogicalSize) {
        const height = Math.min(620, maxH);
        await win.setSize(new tw.LogicalSize(WIDTH, height));
      }
    }
  } catch (_) {}
  updatePageScroll();
}

function updatePageScroll() {
  const root = document.documentElement;
  const needsScroll = root.scrollHeight > root.clientHeight + 2;
  document.body.style.overflowY = needsScroll ? "auto" : "hidden";
  document.body.style.overflowX = "hidden";
}

function setWatching(on) {
  $("startBtn").disabled = on;
  $("stopBtn").disabled = !on;
  $("watchState").textContent = on ? "🟢 Watching" : "⚪ Not watching";
}

function autoSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    const config = getConfig();
    if (!config.host && !config.user && !config.password && !config.localPaths.LGD && !config.localPaths.NATURAL) {
      return;
    }
    window.api.saveSettings(config).catch(() => {});
  }, 400);
}

function typesForRow(i) {
  const taken = new Set(
    watchRows.filter((_, j) => j !== i).map((row) => row.type)
  );
  return PRODUCT_TYPES.filter((t) => t === watchRows[i].type || !taken.has(t));
}

function fillTypeSelect(select, i) {
  const current = watchRows[i].type;
  const allowed = typesForRow(i);
  select.innerHTML = "";
  for (const t of allowed) {
    const opt = document.createElement("option");
    opt.value = t;
    opt.textContent = t;
    select.appendChild(opt);
  }
  select.value = current;
}

function refreshTypeOptions() {
  $("watchRows").querySelectorAll(".typeSelect").forEach((select, i) => {
    fillTypeSelect(select, i);
  });
  $("addTypeBtn").disabled = !unusedType();
}

function renderWatchRows() {
  const box = $("watchRows");
  box.innerHTML = "";
  watchRows.forEach((row, i) => {
    const wrap = document.createElement("div");
    wrap.className = "watchRow";
    wrap.dataset.i = String(i);

    const typeBox = document.createElement("div");
    typeBox.className = "watchRowType";
    const typeSelect = document.createElement("select");
    typeSelect.className = "typeSelect";
    typeSelect.dataset.i = String(i);
    fillTypeSelect(typeSelect, i);
    typeBox.appendChild(typeSelect);

    const pathBox = document.createElement("div");
    pathBox.className = "watchRowPath";
    const pathRow = document.createElement("div");
    pathRow.className = "pathRow";

    const pathInput = document.createElement("input");
    pathInput.className = "pathInput";
    pathInput.dataset.i = String(i);
    pathInput.placeholder = "Select a CSV/Excel file or a folder...";
    pathInput.value = row.path;

    const fileBtn = document.createElement("button");
    fileBtn.type = "button";
    fileBtn.className = "btn-grey pickFile";
    fileBtn.dataset.i = String(i);
    fileBtn.title = "Browse file";
    fileBtn.textContent = "File…";

    const folderBtn = document.createElement("button");
    folderBtn.type = "button";
    folderBtn.className = "btn-grey pickFolder";
    folderBtn.dataset.i = String(i);
    folderBtn.title = "Browse folder";
    folderBtn.textContent = "Folder…";

    pathRow.appendChild(pathInput);
    pathRow.appendChild(fileBtn);
    pathRow.appendChild(folderBtn);

    if (watchRows.length > 1) {
      const removeBtn = document.createElement("button");
      removeBtn.type = "button";
      removeBtn.className = "btn-red removeRow";
      removeBtn.dataset.i = String(i);
      removeBtn.title = "Remove this type";
      removeBtn.textContent = "✕";
      pathRow.appendChild(removeBtn);
    }

    pathBox.appendChild(pathRow);
    wrap.appendChild(typeBox);
    wrap.appendChild(pathBox);
    box.appendChild(wrap);
  });
  refreshTypeOptions();
  updatePageScroll();
}

$("protocol").addEventListener("change", () => {
  $("port").value = DEFAULT_PORTS[$("protocol").value];
  autoSave();
});

for (const id of ["host", "port", "user", "password"]) {
  $(id).addEventListener("input", autoSave);
  $(id).addEventListener("change", autoSave);
}

$("watchRows").addEventListener("change", (e) => {
  const select = e.target.closest(".typeSelect");
  if (!select) return;
  const i = Number(select.dataset.i);
  const next = select.value === "NATURAL" ? "NATURAL" : "LGD";
  if (watchRows.some((row, j) => j !== i && row.type === next)) {
    select.value = watchRows[i].type;
    return;
  }
  watchRows[i].type = next;
  refreshTypeOptions();
  autoSave();
});

$("watchRows").addEventListener("input", (e) => {
  const input = e.target.closest(".pathInput");
  if (!input) return;
  watchRows[Number(input.dataset.i)].path = input.value.trim();
  autoSave();
});

$("watchRows").addEventListener("click", async (e) => {
  const btn = e.target.closest("button");
  if (!btn) return;
  const i = Number(btn.dataset.i);
  if (Number.isNaN(i) || !watchRows[i]) return;

  if (btn.classList.contains("pickFile") || btn.classList.contains("pickFolder")) {
    const kind = btn.classList.contains("pickFolder") ? "folder" : "file";
    const p = await window.api.choosePath(kind);
    if (!p) return;
    watchRows[i].path = p;
    const input = $("watchRows").querySelector(`.pathInput[data-i="${i}"]`);
    if (input) input.value = p;
    autoSave();
    return;
  }

  if (btn.classList.contains("removeRow") && watchRows.length > 1) {
    watchRows.splice(i, 1);
    renderWatchRows();
    autoSave();
  }
});

$("addTypeBtn").addEventListener("click", () => {
  const type = unusedType();
  if (!type) return;
  watchRows.push({ type, path: "" });
  renderWatchRows();
  autoSave();
});

$("testBtn").addEventListener("click", async () => {
  const config = getConfig();
  if (!config.host || !config.user) return setStatus(false, "Please enter host and user name.");
  $("testBtn").disabled = true;
  setStatus(true, "Connecting… testing login, then /LGD and /NATURAL…");
  try {
    const result = await window.api.testFtp(config);
    const ok = !!(result && result.ok);
    const message = (result && result.message) || (ok ? "Connected." : "Test failed.");
    setStatus(ok, message);
    addLog({
      type: ok ? "ok" : "error",
      message: ok ? `Test OK: ${message}` : `Test failed: ${message}`,
      time: new Date().toLocaleTimeString()
    });
  } catch (err) {
    const message = (err && err.message) ? err.message : String(err);
    setStatus(false, message);
    addLog({
      type: "error",
      message: `Test failed: ${message}`,
      time: new Date().toLocaleTimeString()
    });
  } finally {
    $("testBtn").disabled = false;
  }
});

$("startBtn").addEventListener("click", async () => {
  const config = getConfig();
  if (!config.localPaths.LGD && !config.localPaths.NATURAL) {
    return setStatus(false, "Please select a file or folder to watch.");
  }
  const result = await window.api.startWatch(config);
  const ok = result === true || (result && result.ok);
  if (ok) {
    setWatching(true);
    if (result && result.message) setStatus(true, result.message);
  } else {
    setStatus(false, (result && result.message) || "Could not start watching.");
  }
});

$("stopBtn").addEventListener("click", async () => {
  await window.api.stopWatch();
  setWatching(false);
});

renderWatchRows();
layoutWindow();
window.addEventListener("resize", updatePageScroll);

const GUIDE_TARGETS = [
  ".protocol",
  ["#host", "#port", "#user", "#password"],
  "#testBtn",
  "#watchRows",
  "#addTypeBtn",
  "#startBtn"
];

const GUIDE_I18N = {
  en: {
    previous: "Previous",
    next: "Next",
    done: "Done",
    close: "Close",
    steps: [
      {
        title: "1. Connection type",
        hint: "Click Connection Type",
        text: "Open the Connection Type menu and pick FTP, FTPS, or SFTP. The port fills in automatically (21, 990, or 22)."
      },
      {
        title: "2. Login details",
        hint: "Type host, user, and password",
        text: "Enter Host Name, User Name, and Password. Settings save as you type, so you will not need to re-enter them after an update."
      },
      {
        title: "3. Test FTP",
        hint: "Click the blue Test FTP button",
        text: "Click Test FTP to confirm the login. A green message means the server is reachable. If it fails, check host, user, password, or try another connection type."
      },
      {
        title: "4. File or folder to watch",
        hint: "Click File… or Folder…",
        text: "Choose LGD or NATURAL, then click File… for one CSV/Excel file or Folder… for a whole folder. Uploads go to /LGD or /NATURAL on the same FTP account."
      },
      {
        title: "5. Add the other type (optional)",
        hint: "Click + Add another type",
        text: "Need both LGD and NATURAL? Click + Add another type. That row gets its own path. Same FTP login is used for both."
      },
      {
        title: "6. Start Watching",
        hint: "Click the green Start Watching button",
        text: "Existing files upload first. After that, a changed file waits 30 seconds, then uploads. The Activity Log shows upload %. At 100% the progress bar hides."
      }
    ]
  },
  hi: {
    previous: "पिछला",
    next: "अगला",
    done: "पूरा",
    close: "बंद",
    steps: [
      {
        title: "1. कनेक्शन प्रकार",
        hint: "Connection Type पर क्लिक करें",
        text: "Connection Type मेनू खोलें और FTP, FTPS, या SFTP चुनें। पोर्ट अपने आप भर जाता है (21, 990, या 22)।"
      },
      {
        title: "2. लॉगिन विवरण",
        hint: "Host, User और Password टाइप करें",
        text: "Host Name, User Name और Password भरें। सेटिंग्स टाइप करते ही सेव हो जाती हैं, अपडेट के बाद दोबारा भरने की ज़रूरत नहीं।"
      },
      {
        title: "3. Test FTP",
        hint: "नीले Test FTP बटन पर क्लिक करें",
        text: "लॉगिन जाँचने के लिए Test FTP दबाएँ। हरा संदेश मतलब सर्वर मिल गया। फेल हो तो host, user, password जाँचें या दूसरा कनेक्शन प्रकार आज़माएँ।"
      },
      {
        title: "4. फ़ाइल या फ़ोल्डर देखें",
        hint: "File… या Folder… पर क्लिक करें",
        text: "LGD या NATURAL चुनें, फिर एक CSV/Excel के लिए File… या पूरे फ़ोल्डर के लिए Folder…। अपलोड उसी FTP अकाउंट पर /LGD या /NATURAL में जाते हैं।"
      },
      {
        title: "5. दूसरा प्रकार जोड़ें (वैकल्पिक)",
        hint: "+ Add another type पर क्लिक करें",
        text: "LGD और NATURAL दोनों चाहिए? + Add another type दबाएँ। उस पंक्ति का अपना पाथ होगा। दोनों के लिए वही FTP लॉगिन।"
      },
      {
        title: "6. Start Watching",
        hint: "हरे Start Watching बटन पर क्लिक करें",
        text: "पहले मौजूदा फ़ाइलें अपलोड होती हैं। उसके बाद बदली हुई फ़ाइल 30 सेकंड बाद अपलोड होती है। Activity Log में अपलोड % दिखता है। 100% पर प्रोग्रेस बार छिप जाता है।"
      }
    ]
  },
  gu: {
    previous: "પહેલાં",
    next: "આગળ",
    done: "પૂર્ણ",
    close: "બંધ",
    steps: [
      {
        title: "1. કનેક્શન પ્રકાર",
        hint: "Connection Type પર ક્લિક કરો",
        text: "Connection Type મેનૂ ખોલો અને FTP, FTPS, અથવા SFTP પસંદ કરો. પોર્ટ આપમેળે ભરાય છે (21, 990, અથવા 22)."
      },
      {
        title: "2. લોગિન વિગતો",
        hint: "Host, User અને Password લખો",
        text: "Host Name, User Name અને Password ભરો. સેટિંગ્સ ટાઈપ કરતાં જ સેવ થાય છે, અપડેટ પછી ફરી ભરવાની જરૂર નથી."
      },
      {
        title: "3. Test FTP",
        hint: "વાદળી Test FTP બટન પર ક્લિક કરો",
        text: "લોગિન ચકાસવા Test FTP દબાવો. લીલો સંદેશ એટલે સર્વર મળ્યું. નિષ્ફળ થાય તો host, user, password તપાસો અથવા બીજો કનેક્શન પ્રકાર અજમાવો."
      },
      {
        title: "4. ફાઈલ અથવા ફોલ્ડર જુઓ",
        hint: "File… અથવા Folder… પર ક્લિક કરો",
        text: "LGD અથવા NATURAL પસંદ કરો, પછી એક CSV/Excel માટે File… અથવા આખા ફોલ્ડર માટે Folder…. અપલોડ એ જ FTP અકાઉન્ટ પર /LGD અથવા /NATURAL માં જાય છે."
      },
      {
        title: "5. બીજો પ્રકાર ઉમેરો (વૈકલ્પિક)",
        hint: "+ Add another type પર ક્લિક કરો",
        text: "LGD અને NATURAL બંને જોઈએ? + Add another type દબાવો. તે પંક્તિનો પોતાનો પાથ હશે. બંને માટે એ જ FTP લોગિન."
      },
      {
        title: "6. Start Watching",
        hint: "લીલા Start Watching બટન પર ક્લિક કરો",
        text: "પહેલા હાલની ફાઈલો અપલોડ થાય છે. પછી બદલાયેલી ફાઈલ 30 સેકન્ડ પછી અપલોડ થાય છે. Activity Logમાં અપલોડ % દેખાય છે. 100% પર પ્રોગ્રેસ બાર છુપાય છે."
      }
    ]
  }
};

function detectGuideLang() {
  try {
    const saved = localStorage.getItem("lgdGuideLang");
    if (saved && GUIDE_I18N[saved]) return saved;
  } catch (_) {}
  const loc = String(navigator.language || "en").toLowerCase();
  if (loc.startsWith("hi")) return "hi";
  if (loc.startsWith("gu")) return "gu";
  return "en";
}

let guideLang = detectGuideLang();

let guideIndex = 0;
let guideOpen = false;

function clearSpot() {
  document.querySelectorAll(".guide-spot").forEach((el) => el.classList.remove("guide-spot"));
}

function resolveSpot(el) {
  if (el.matches("input, select") && el.parentElement) return el.parentElement;
  return el;
}

function spot(sel) {
  clearSpot();
  const sels = Array.isArray(sel) ? sel : [sel];
  sels.forEach((s) => {
    const el = s && document.querySelector(s);
    if (el) resolveSpot(el).classList.add("guide-spot");
  });
  const first = document.querySelector(".guide-spot");
  if (first) first.scrollIntoView({ block: "nearest", behavior: "smooth" });
}

function spotRect() {
  const els = [...document.querySelectorAll(".guide-spot")];
  if (!els.length) return null;
  let top = Infinity;
  let left = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (const el of els) {
    const r = el.getBoundingClientRect();
    top = Math.min(top, r.top);
    left = Math.min(left, r.left);
    right = Math.max(right, r.right);
    bottom = Math.max(bottom, r.bottom);
  }
  return { top, left, right, bottom, width: right - left, height: bottom - top };
}

function positionGuideTip() {
  const tip = $("guideTip");
  if (!guideOpen) return;
  const box = spotRect();
  const pad = 12;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const tipW = tip.offsetWidth;
  const tipH = tip.offsetHeight;
  let top;
  let place = "below";
  if (box) {
    if (box.bottom + pad + tipH <= vh - 8) {
      top = box.bottom + pad;
      place = "below";
    } else if (box.top - pad - tipH >= 8) {
      top = box.top - pad - tipH;
      place = "above";
    } else {
      top = Math.max(8, Math.min(box.bottom + pad, vh - tipH - 8));
      place = "below";
    }
  } else {
    top = Math.max(8, (vh - tipH) / 2);
  }
  let left = box ? box.left + box.width / 2 - tipW / 2 : (vw - tipW) / 2;
  left = Math.max(8, Math.min(left, vw - tipW - 8));
  tip.style.top = `${Math.round(top)}px`;
  tip.style.left = `${Math.round(left)}px`;
  tip.dataset.place = place;
  const center = box ? box.left + box.width / 2 : left + tipW / 2;
  const arrow = Math.max(16, Math.min(center - left, tipW - 16));
  tip.style.setProperty("--arrow-x", `${Math.round(arrow)}px`);
}

function openSidebar() {
  closeGuide();
  document.body.classList.add("sidebar-open");
  $("sidebar").setAttribute("aria-hidden", "false");
}

function closeSidebar() {
  document.body.classList.remove("sidebar-open");
  $("sidebar").setAttribute("aria-hidden", "true");
}

function startGuide() {
  closeSidebar();
  guideOpen = true;
  guideIndex = 0;
  document.body.classList.add("guide-on");
  $("guideTip").hidden = false;
  setGuideLang(guideLang);
}

function markGuideSeen() {
  try {
    localStorage.setItem("lgdGuideSeen", "1");
  } catch (_) {}
}

function isFirstInstall() {
  try {
    if (localStorage.getItem("lgdGuideSeen") === "1") return false;
  } catch (_) {}
  return !$("host").value.trim() && !$("user").value.trim();
}

function closeGuide() {
  if (!guideOpen && !document.body.classList.contains("guide-on")) {
    clearSpot();
    return;
  }
  guideOpen = false;
  document.body.classList.remove("guide-on");
  $("guideTip").hidden = true;
  clearSpot();
  markGuideSeen();
}

function setGuideLang(lang) {
  if (!GUIDE_I18N[lang]) lang = "en";
  guideLang = lang;
  try {
    localStorage.setItem("lgdGuideLang", lang);
  } catch (_) {}
  $("guideLangs").querySelectorAll("button").forEach((btn) => {
    btn.classList.toggle("on", btn.dataset.lang === lang);
  });
  $("guideClose").setAttribute("aria-label", GUIDE_I18N[lang].close);
  if (guideOpen) showGuideStep(guideIndex);
}

function showGuideStep(i) {
  const pack = GUIDE_I18N[guideLang] || GUIDE_I18N.en;
  const total = GUIDE_TARGETS.length;
  guideIndex = Math.max(0, Math.min(total - 1, i));
  const step = pack.steps[guideIndex];
  $("guideTipTitle").textContent = step.title;
  $("guideHint").textContent = step.hint;
  $("guideText").textContent = step.text;
  $("guideCount").textContent = `${guideIndex + 1} / ${total}`;
  $("guideBack").disabled = guideIndex === 0;
  $("guideBack").textContent = pack.previous;
  $("guideNext").textContent = guideIndex === total - 1 ? pack.done : pack.next;
  spot(GUIDE_TARGETS[guideIndex]);
  requestAnimationFrame(() => {
    positionGuideTip();
    setTimeout(positionGuideTip, 220);
  });
}

$("guideBtn").addEventListener("click", () => {
  if (guideOpen) closeGuide();
  else startGuide();
});
$("faqBtn").addEventListener("click", openSidebar);
$("sidebarClose").addEventListener("click", closeSidebar);
$("sidebarScrim").addEventListener("click", closeSidebar);
$("guideClose").addEventListener("click", closeGuide);
$("guideScrim").addEventListener("click", closeGuide);
$("guideBack").addEventListener("click", (e) => {
  e.stopPropagation();
  showGuideStep(guideIndex - 1);
});
$("guideNext").addEventListener("click", (e) => {
  e.stopPropagation();
  if (guideIndex >= GUIDE_TARGETS.length - 1) closeGuide();
  else showGuideStep(guideIndex + 1);
});
$("guideLangs").addEventListener("click", (e) => {
  const btn = e.target.closest("button[data-lang]");
  if (!btn) return;
  e.stopPropagation();
  setGuideLang(btn.dataset.lang);
});
$("guideTip").addEventListener("click", (e) => e.stopPropagation());
window.addEventListener("resize", () => {
  if (guideOpen) positionGuideTip();
});
document.addEventListener("keydown", (e) => {
  if (e.key !== "Escape") return;
  if (guideOpen) closeGuide();
  else closeSidebar();
});

let splashDone = false;
let configReady = false;

function tryAutoGuide() {
  if (!splashDone || !configReady) return;
  if (isFirstInstall()) startGuide();
}

// Splash overlay: play the intro, then fade smoothly into the app UI.
setTimeout(() => {
  const splash = $("splash");
  splash.classList.add("fadeOut");
  setTimeout(() => {
    splash.remove();
    layoutWindow();
    splashDone = true;
    tryAutoGuide();
  }, 800);
}, 2200);

window.api.onLog(addLog);
window.api.onWatchState((on) => {
  setWatching(on);
  if (on) setStatus(true, "Watch mode resumed automatically.");
});
window.api.loadConfig().then((c) => {
  setConfig(c);
  if (c) {
    if (c.watching) setWatching(true);
    setStatus(true, "Saved settings loaded.");
    addLog({
      type: "ok",
      message: "Existing FTP details loaded — no need to re-enter them.",
      time: new Date().toLocaleTimeString()
    });
  }
  configReady = true;
  tryAutoGuide();
}).catch(() => {
  configReady = true;
  tryAutoGuide();
});
