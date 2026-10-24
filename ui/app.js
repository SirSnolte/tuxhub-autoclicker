// Interface of the autoclicker, shown by tuxhub and by the tool's own window;
// both provide window.tuxhub. Settings, devices and macros live in the
// background service. Texts follow the language reported by the host.

"use strict";

const STRINGS = {
  en: {
    "notice.anticheat":
      "Automated input can violate the terms of service of online games, in particular games with anti-cheat software. Use it at your own risk.",
    "tab.clicker": "Autoclicker",
    "tab.macros": "Macros",
    "clicker.start": "Start",
    "clicker.stop": "Stop",
    "clicker.hotkeys": "F8 starts or stops immediately. F10 stops everything.",
    "clicker.hotkeysStandalone":
      "Hotkeys (F8, F10 and your own) work while tuxhub is running. Here, stop with the button or by closing the window.",
    "clicker.stopKey": "Stop hotkey",
    "clicker.stopKeyHelp": "Optional key that only stops the clicker, in addition to F8 and F10.",
    "clicker.setupHint": "Nothing happens?",
    "clicker.setup": "Set up input device access",
    "field.interval_ms": "Interval",
    "help.interval_ms": "Time between two clicks",
    "field.jitter_pct": "Random variation",
    "field.button": "Mouse button",
    "field.mode": "Mode",
    "field.limit": "Number of clicks",
    "help.limit": "0 = unlimited",
    "field.start_delay_s": "Start delay",
    "help.start_delay_s": "Applies to Start only. The hotkey starts immediately.",
    "field.record_moves": "Record mouse movements in macros",
    "help.record_moves": "When off, only keys, clicks and the mouse wheel are recorded.",
    "button.left": "Left",
    "button.right": "Right",
    "button.middle": "Middle",
    "mode.click": "Single click",
    "mode.double": "Double click",
    "mode.hold": "Hold",
    "status.ready": "Ready",
    "status.countdown": "Starting in {seconds} s",
    "status.clicking": "Clicking: {clicks} clicks",
    "status.holding": "Holding the button",
    "status.stopped": "Stopped after {clicks} clicks",
    "status.error": "Error: {error}",
    "key.none": "None",
    "key.set": "Set key",
    "key.clear": "Remove",
    "key.capture": "Press a key (Esc to cancel)",
    "key.unsupported": "The key “{key}” is not supported.",
    "key.needsModifier":
      "This key is needed for typing or playing. Combine it with Ctrl, Alt or Super.",
    "macros.record": "Start recording",
    "macros.stopRecording": "Stop recording",
    "macros.recordHint": "or press F9 in the game, and again to finish.",
    "macros.new": "New macro",
    "macros.privacy": "Every key is stored while recording. Do not type passwords during a recording.",
    "macros.name": "Name",
    "macros.steps": "Steps",
    "macros.duration": "Duration",
    "macros.repeat": "Repeat",
    "macros.hotkey": "Hotkey",
    "macros.empty": "No macros yet. Start a recording with F9.",
    "macros.play": "Play",
    "macros.stop": "Stop",
    "macros.edit": "Edit",
    "macros.delete": "Delete",
    "macros.confirmDelete": "Delete “{name}”?",
    "macros.endless": "Endless",
    "macros.times": "{count}×",
    "macros.recording": "Recording from {devices} devices",
    "macros.saved": "“{name}” saved with {steps} steps",
    "macros.nothing": "Nothing was recorded",
    "macros.discarded": "Recording discarded",
    "macros.playing": "Playing “{name}”, round {round}",
    "macros.playingOf": "Playing “{name}”, round {round} of {rounds}",
    "macros.finished": "“{name}” finished",
    "macros.stopped": "“{name}” stopped",
    "macros.defaultName": "New macro",
    "editor.titleEdit": "Edit macro",
    "editor.titleNew": "New macro",
    "editor.cancel": "Cancel",
    "editor.save": "Save",
    "editor.name": "Name",
    "editor.repeat": "Repetitions (0 = endless)",
    "editor.speed": "Speed",
    "editor.hotkey": "Hotkey",
    "editor.add": "Add step:",
    "editor.tap": "Tap",
    "editor.press": "Press",
    "editor.release": "Release",
    "editor.wait": "Wait",
    "editor.scale": "Scale delays",
    "editor.scaleHelp": "Multiply all delays, for example 0.5 to play twice as fast",
    "editor.scalePrompt": "Multiply all delays by (for example 0.5 = twice as fast):",
    "editor.delay": "Delay (ms)",
    "editor.action": "Action",
    "editor.summary": "{steps} steps, {duration} per round",
    "editor.moveUp": "Move up",
    "editor.moveDown": "Move down",
    "editor.remove": "Remove",
    "step.press": "Press {key}",
    "step.release": "Release {key}",
    "step.move": "Move mouse by ({dx}, {dy})",
    "step.scrollUp": "Scroll up ({n})",
    "step.scrollDown": "Scroll down ({n})",
    "step.scrollSide": "Scroll sideways ({n})",
    "step.wait": "Wait",
    "mouse": "Mouse",
    "macros.actions": "Actions",
    "macros.deleteHint": "This cannot be undone.",
    "editor.key": "Key",
    "editor.apply": "Apply",
    "error.dismiss": "Dismiss",
  },
  de: {
    "notice.anticheat":
      "Automatisierte Eingaben können gegen die Nutzungsbedingungen von Online-Spielen verstoßen, insbesondere bei Spielen mit Anti-Cheat-Software. Die Nutzung erfolgt auf eigenes Risiko.",
    "tab.clicker": "Autoclicker",
    "tab.macros": "Makros",
    "clicker.start": "Start",
    "clicker.stop": "Stopp",
    "clicker.hotkeys": "F8 startet oder stoppt sofort. F10 stoppt alles.",
    "clicker.hotkeysStandalone":
      "Hotkeys (F8, F10 und eigene) funktionieren, solange tuxhub läuft. Hier stoppst du mit der Schaltfläche oder indem du das Fenster schließt.",
    "clicker.stopKey": "Stopp-Taste",
    "clicker.stopKeyHelp": "Optionale Taste, die nur den Klicker stoppt, zusätzlich zu F8 und F10.",
    "clicker.setupHint": "Es passiert nichts?",
    "clicker.setup": "Zugriff auf Eingabegeräte einrichten",
    "field.interval_ms": "Intervall",
    "help.interval_ms": "Zeit zwischen zwei Klicks",
    "field.jitter_pct": "Zufällige Abweichung",
    "field.button": "Maustaste",
    "field.mode": "Modus",
    "field.limit": "Anzahl Klicks",
    "help.limit": "0 = unbegrenzt",
    "field.start_delay_s": "Startverzögerung",
    "help.start_delay_s": "Gilt nur für Start. Der Hotkey startet sofort.",
    "field.record_moves": "Mausbewegungen in Makros aufnehmen",
    "help.record_moves": "Wenn aus, werden nur Tasten, Klicks und das Mausrad aufgenommen.",
    "button.left": "Links",
    "button.right": "Rechts",
    "button.middle": "Mitte",
    "mode.click": "Einzelklick",
    "mode.double": "Doppelklick",
    "mode.hold": "Gedrückt halten",
    "status.ready": "Bereit",
    "status.countdown": "Start in {seconds} s",
    "status.clicking": "Klickt: {clicks} Klicks",
    "status.holding": "Taste wird gehalten",
    "status.stopped": "Gestoppt nach {clicks} Klicks",
    "status.error": "Fehler: {error}",
    "key.none": "Keine",
    "key.set": "Taste festlegen",
    "key.clear": "Entfernen",
    "key.capture": "Taste drücken (Esc bricht ab)",
    "key.unsupported": "Die Taste „{key}“ wird nicht unterstützt.",
    "key.needsModifier":
      "Diese Taste wird zum Tippen oder Spielen gebraucht. Bitte mit Strg, Alt oder Super kombinieren.",
    "macros.record": "Aufnahme starten",
    "macros.stopRecording": "Aufnahme beenden",
    "macros.recordHint": "oder F9 im Spiel drücken, erneut drücken zum Beenden.",
    "macros.new": "Neues Makro",
    "macros.privacy": "Während der Aufnahme wird jede Taste gespeichert. Bitte keine Passwörter eingeben.",
    "macros.name": "Name",
    "macros.steps": "Schritte",
    "macros.duration": "Dauer",
    "macros.repeat": "Wiederholung",
    "macros.hotkey": "Hotkey",
    "macros.empty": "Noch keine Makros. Starte eine Aufnahme mit F9.",
    "macros.play": "Abspielen",
    "macros.stop": "Stopp",
    "macros.edit": "Bearbeiten",
    "macros.delete": "Löschen",
    "macros.confirmDelete": "„{name}“ löschen?",
    "macros.endless": "Endlos",
    "macros.times": "{count}×",
    "macros.recording": "Aufnahme von {devices} Geräten",
    "macros.saved": "„{name}“ mit {steps} Schritten gespeichert",
    "macros.nothing": "Es wurde nichts aufgenommen",
    "macros.discarded": "Aufnahme verworfen",
    "macros.playing": "Spielt „{name}“, Runde {round}",
    "macros.playingOf": "Spielt „{name}“, Runde {round} von {rounds}",
    "macros.finished": "„{name}“ beendet",
    "macros.stopped": "„{name}“ gestoppt",
    "macros.defaultName": "Neues Makro",
    "editor.titleEdit": "Makro bearbeiten",
    "editor.titleNew": "Neues Makro",
    "editor.cancel": "Abbrechen",
    "editor.save": "Speichern",
    "editor.name": "Name",
    "editor.repeat": "Wiederholungen (0 = endlos)",
    "editor.speed": "Geschwindigkeit",
    "editor.hotkey": "Hotkey",
    "editor.add": "Schritt hinzufügen:",
    "editor.tap": "Tippen",
    "editor.press": "Drücken",
    "editor.release": "Loslassen",
    "editor.wait": "Warten",
    "editor.scale": "Zeiten skalieren",
    "editor.scaleHelp": "Alle Wartezeiten multiplizieren, zum Beispiel 0,5 für doppelte Geschwindigkeit",
    "editor.scalePrompt": "Alle Wartezeiten multiplizieren mit (zum Beispiel 0.5 = doppelt so schnell):",
    "editor.delay": "Warten (ms)",
    "editor.action": "Aktion",
    "editor.summary": "{steps} Schritte, {duration} pro Durchlauf",
    "editor.moveUp": "Nach oben",
    "editor.moveDown": "Nach unten",
    "editor.remove": "Entfernen",
    "step.press": "{key} drücken",
    "step.release": "{key} loslassen",
    "step.move": "Maus bewegen um ({dx}, {dy})",
    "step.scrollUp": "Mausrad hoch ({n})",
    "step.scrollDown": "Mausrad runter ({n})",
    "step.scrollSide": "Mausrad seitlich ({n})",
    "step.wait": "Warten",
    "mouse": "Maus",
    "macros.actions": "Aktionen",
    "macros.deleteHint": "Das lässt sich nicht rückgängig machen.",
    "editor.key": "Taste",
    "editor.apply": "Übernehmen",
    "error.dismiss": "Schließen",
  },
};

let locale = navigator.language?.toLowerCase().startsWith("de") ? "de" : "en";

function t(key, vars = {}) {
  let s = STRINGS[locale][key] ?? STRINGS.en[key] ?? key;
  for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

const $ = (id) => document.getElementById(id);
const esc = (s) =>
  String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
// Errors appear in the page instead of in a browser dialog.
const fail = (e) => {
  $("error-text").textContent = String(e?.message ?? e);
  $("error").classList.remove("hidden");
};
$("error-close").onclick = () => $("error").classList.add("hidden");

// Notices stay hidden once dismissed. Storage can be unavailable; the notice
// then only hides until the page is reloaded.
for (const el of document.querySelectorAll("[data-dismiss]")) {
  const key = `dismissed.${el.dataset.dismiss}`;
  try {
    if (localStorage.getItem(key)) el.classList.add("hidden");
  } catch {}
  el.querySelector("wa-button").onclick = () => {
    el.classList.add("hidden");
    try {
      localStorage.setItem(key, "1");
    } catch {}
  };
}
const fmtMs = (ms) => (ms >= 1000 ? (ms / 1000).toFixed(1) + " s" : ms + " ms");

// Last notifications, kept so texts can be rebuilt when the language changes.
let clickerStatus = null;
let macroStatus = null;

function applyStaticTexts() {
  document.documentElement.lang = locale;
  for (const el of document.querySelectorAll("[data-i18n]")) el.textContent = t(el.dataset.i18n);
  for (const el of document.querySelectorAll("[data-i18n-title]")) el.title = t(el.dataset.i18nTitle);
  for (const el of document.querySelectorAll("[data-i18n-label]")) el.label = t(el.dataset.i18nLabel);
  for (const el of document.querySelectorAll("[data-i18n-aria-label]")) el.setAttribute("aria-label", t(el.dataset.i18nAriaLabel));
}

/**
 * Asks in the page's own dialog instead of a browser dialog. Resolves to true
 * (or the entered value when `value` is given) on confirmation, else null.
 */
function ask({ title, text = "", ok, danger = false, value = null }) {
  const dialog = $("dialog");
  const input = $("dialog-input");
  dialog.label = title;
  $("dialog-text").textContent = text;
  $("dialog-text").classList.toggle("hidden", !text);
  $("dialog-ok").textContent = ok;
  $("dialog-ok").variant = danger ? "danger" : "brand";
  input.classList.toggle("hidden", value === null);
  if (value !== null) input.value = value;
  return new Promise((resolve) => {
    const cleanup = () => {
      $("dialog-ok").removeEventListener("click", onOk);
      $("dialog-cancel").removeEventListener("click", onCancel);
      dialog.removeEventListener("wa-hide", onHide);
    };
    const finish = (result) => {
      cleanup();
      dialog.open = false;
      resolve(result);
    };
    const onOk = () => finish(value === null ? true : input.value);
    const onCancel = () => finish(null);
    // Escape, the close button or a click outside.
    const onHide = (e) => {
      if (e.target !== dialog) return;
      cleanup();
      resolve(null);
    };
    $("dialog-ok").addEventListener("click", onOk);
    $("dialog-cancel").addEventListener("click", onCancel);
    dialog.addEventListener("wa-hide", onHide);
    dialog.open = true;
  });
}

function applyTheme(dark) {
  document.documentElement.classList.toggle("wa-dark", dark);
  document.documentElement.classList.toggle("wa-light", !dark);
}
applyTheme(matchMedia("(prefers-color-scheme: dark)").matches);
tuxhub.on("theme", ({ dark }) => applyTheme(dark));

function rerender() {
  applyStaticTexts();
  renderForm();
  showStopKey();
  renderClickerStatus();
  renderMacroStatus();
  renderMacros();
  fillKeySelect();
  if (draft) renderSteps();
  if (draft) $("ed-title").textContent = draft.id ? t("editor.titleEdit") : t("editor.titleNew");
}

tuxhub.on("locale", ({ locale: l }) => {
  locale = l === "de" ? "de" : "en";
  rerender();
});

// ------------------------------------------------------------------- Tabs

$("tabs").addEventListener("wa-tab-show", (e) => {
  if (e.detail?.name === "macros") loadMacros();
});
for (const b of document.querySelectorAll("[data-action]")) {
  b.onclick = () => tuxhub.action(b.dataset.action).catch(fail);
}

// ----------------------------------------------------------------- Clicker

const FIELDS = [
  { key: "interval_ms", type: "number", min: 1, max: 60000, unit: "ms", help: true },
  { key: "jitter_pct", type: "number", min: 0, max: 50, unit: "%" },
  { key: "button", type: "select", options: ["left", "right", "middle"] },
  { key: "mode", type: "select", options: ["click", "double", "hold"] },
  { key: "limit", type: "number", min: 0, max: 1000000, help: true },
  { key: "start_delay_s", type: "number", min: 0, max: 30, unit: "s", help: true },
  { key: "record_moves", type: "toggle", help: true },
];

let settings = {};
let pending = {};
let saveTimer;

// Changes are collected briefly and sent to the service together.
function save(key, value) {
  settings[key] = value;
  pending[key] = value;
  clearTimeout(saveTimer);
  saveTimer = setTimeout(sendPending, 300);
}

function sendPending() {
  const patch = pending;
  pending = {};
  tuxhub.call("config.set", patch).catch((e) => {
    fail(e);
    loadSettings();
  });
}

function applySettings(c) {
  const changed = JSON.stringify(c) !== JSON.stringify(settings);
  settings = c;
  if (changed) {
    renderForm();
    showStopKey();
  }
}

function loadSettings() {
  return tuxhub.call("config.get").then(applySettings).catch(fail);
}

// Another window, tuxhub or the hotkey editor changed the settings.
tuxhub.on("config", (c) => {
  if (Object.keys(pending).length === 0) applySettings(c);
});

function slotted(slot, text, className) {
  return Object.assign(document.createElement("span"), { slot, textContent: text, className: className ?? "" });
}

// Each field is a settings row as in tuxhub (class "setting", see theme.css).
function renderForm() {
  const form = $("clicker-form");
  form.replaceChildren();
  for (const f of FIELDS) {
    const label = t(`field.${f.key}`);
    const help = f.help ? t(`help.${f.key}`) : null;
    let input;
    if (f.type === "select") {
      input = document.createElement("wa-select");
      input.append(slotted("label", label));
      for (const v of f.options) {
        input.append(Object.assign(document.createElement("wa-option"), { value: v, textContent: t(`${f.key}.${v}`) }));
      }
      input.value = settings[f.key];
      input.addEventListener("change", () => save(f.key, input.value));
    } else if (f.type === "toggle") {
      input = document.createElement("wa-switch");
      input.append(label);
      input.checked = !!settings[f.key];
      input.addEventListener("change", () => save(f.key, input.checked));
    } else {
      input = document.createElement("wa-input");
      Object.assign(input, { type: "number", min: f.min, max: f.max, withoutSpinButtons: true });
      input.append(slotted("label", label));
      if (f.unit) input.append(slotted("end", f.unit, "muted"));
      input.value = settings[f.key];
      input.addEventListener("change", () => {
        let n = Math.round(Number(input.value));
        if (!Number.isFinite(n)) n = f.min;
        n = Math.min(f.max, Math.max(f.min, n));
        input.value = n;
        save(f.key, n);
      });
    }
    input.classList.add("setting");
    if (help) input.append(slotted("hint", help));
    form.append(input);
  }
}

function renderClickerStatus() {
  const s = clickerStatus;
  $("clicker-status").textContent = s ? t(`status.${s.phase}`, { clicks: s.clicks, seconds: s.seconds, error: s.error ?? "" }) : "";
}

function showClickerStatus(s) {
  clickerStatus = s;
  renderClickerStatus();
}
tuxhub.on("status", showClickerStatus);
// The service may have reported its state before this page was loaded.
tuxhub.call("status").then(showClickerStatus).catch(fail);

// ------------------------------------------------------------ Hotkey input

// Converts a key event into the XDG shortcut format (CTRL+SHIFT+q, F7, ...).
const NAMED = {
  Pause: "Pause",
  ScrollLock: "Scroll_Lock",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "Page_Up",
  PageDown: "Page_Down",
  Delete: "Delete",
  Escape: "Escape",
  " ": "space",
  Enter: "Return",
  Tab: "Tab",
  Backspace: "BackSpace",
};
const STANDALONE = new Set(["Pause", "Scroll_Lock", "Insert", "Home", "End", "Page_Up", "Page_Down", "Delete"]);

function toTrigger(e) {
  if (["Control", "Alt", "Shift", "Meta", "AltGraph", "OS"].includes(e.key)) return { wait: true };
  let key = null;
  if (/^F([1-9]|1\d|2[0-4])$/.test(e.key)) key = e.key;
  else if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3).toLowerCase();
  else if (/^Digit\d$/.test(e.code)) key = e.code.slice(5);
  else key = NAMED[e.key] ?? null;
  if (!key) return { error: t("key.unsupported", { key: e.key }) };
  const mods = [];
  if (e.ctrlKey) mods.push("CTRL");
  if (e.altKey) mods.push("ALT");
  if (e.shiftKey) mods.push("SHIFT");
  if (e.metaKey) mods.push("LOGO");
  const strong = mods.some((m) => m !== "SHIFT");
  if (!strong && !/^F\d+$/.test(key) && !STANDALONE.has(key)) return { error: t("key.needsModifier") };
  return { trigger: [...mods, key].join("+") };
}

/** Waits for the next key press. Escape cancels. */
function captureKey(button) {
  return new Promise((resolve) => {
    const old = button.textContent;
    button.textContent = t("key.capture");
    button.classList.add("capturing");
    const done = (value) => {
      window.removeEventListener("keydown", onKey, true);
      button.textContent = old;
      button.classList.remove("capturing");
      resolve(value);
    };
    const onKey = (e) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey) return done(null);
      const r = toTrigger(e);
      if (r.wait) return;
      if (r.error) {
        fail(r.error);
        return done(null);
      }
      done(r.trigger);
    };
    window.addEventListener("keydown", onKey, true);
    window.focus();
  });
}

function showStopKey() {
  $("stop-key").textContent = settings.stop_shortcut || t("key.none");
}
$("stop-set").onclick = async () => {
  const trigger = await captureKey($("stop-set"));
  if (!trigger) return;
  save("stop_shortcut", trigger);
  showStopKey();
};
$("stop-clear").onclick = () => {
  save("stop_shortcut", "");
  showStopKey();
};

loadSettings();

// ------------------------------------------------------------------ Macros

let macros = [];
let recording = false;
let playing = null;
let keys = [];
let keyNames = new Map();

tuxhub.call("keys").then((list) => {
  keys = list;
  keyNames = new Map(list);
  fillKeySelect();
});

function keyLabel(code) {
  const n = keyNames.get(code) ?? `#${code}`;
  return n.startsWith("BTN_") ? `${t("mouse")} ${n.slice(4).toLowerCase()}` : n.replace(/^KEY_/, "");
}

function fillKeySelect() {
  const sel = $("add-key");
  const current = sel.value || "30"; // KEY_A
  const sorted = [...keys].sort((a, b) => keyLabel(a[0]).localeCompare(keyLabel(b[0]), locale, { numeric: true }));
  sel.replaceChildren(
    ...sorted.map(([code]) =>
      Object.assign(document.createElement("wa-option"), { value: String(code), textContent: keyLabel(code) }),
    ),
  );
  sel.value = String(current);
}

async function loadMacros() {
  macros = await tuxhub.call("macros.list").catch((e) => (fail(e), []));
  renderMacros();
}

function renderMacros() {
  $("empty").classList.toggle("hidden", macros.length > 0);
  $("rec-btn").textContent = recording ? t("macros.stopRecording") : t("macros.record");
  $("rec-btn").variant = recording ? "danger" : "brand";
  $("macro-rows").innerHTML = macros
    .map((m) => {
      const repeat = m.repeat === 0 ? t("macros.endless") : t("macros.times", { count: m.repeat });
      const speed = m.speed !== 1 ? `, ${m.speed}x` : "";
      const hotkey = m.shortcut ? `<kbd>${esc(m.shortcut)}</kbd>` : '<span class="muted">-</span>';
      const control =
        playing === m.id
          ? `<wa-button size="s" variant="danger" data-stop>${esc(t("macros.stop"))}</wa-button>`
          : `<wa-button size="s" variant="brand" data-play="${esc(m.id)}">${esc(t("macros.play"))}</wa-button>`;
      return `<tr class="${playing === m.id ? "playing" : ""}">
        <td class="name">${esc(m.name)}</td>
        <td class="num">${m.steps}</td>
        <td class="num">${fmtMs(m.duration_ms)}</td>
        <td>${esc(repeat + speed)}</td>
        <td>${hotkey}</td>
        <td class="num actions">
          ${control}
          <wa-button size="s" appearance="outlined" data-edit="${esc(m.id)}">${esc(t("macros.edit"))}</wa-button>
          <wa-button size="s" appearance="plain" variant="danger" data-del="${esc(m.id)}">${esc(t("macros.delete"))}</wa-button>
        </td></tr>`;
    })
    .join("");
}

$("macro-rows").onclick = async (e) => {
  const b = e.target.closest("wa-button");
  if (!b) return;
  try {
    if (b.dataset.play) await tuxhub.call("macros.play", { id: b.dataset.play });
    if ("stop" in b.dataset) await tuxhub.call("macros.stop");
    if (b.dataset.edit) openEditor(await tuxhub.call("macros.get", { id: b.dataset.edit }));
    if (b.dataset.del) {
      const m = macros.find((x) => x.id === b.dataset.del);
      const yes = await ask({
        title: t("macros.confirmDelete", { name: m.name }),
        text: t("macros.deleteHint"),
        ok: t("macros.delete"),
        danger: true,
      });
      if (yes) await tuxhub.call("macros.delete", { id: m.id });
    }
  } catch (err) {
    fail(err);
  }
};
$("rec-btn").onclick = () => tuxhub.action("record").catch(fail);
$("new-btn").onclick = () => openEditor({ id: "", name: t("macros.defaultName"), steps: [], repeat: 1, speed: 1 });

function renderMacroStatus() {
  const s = macroStatus;
  const el = $("macro-status");
  if (!s) {
    el.textContent = "";
    return;
  }
  const vars = { name: s.name ?? "", steps: s.steps ?? 0, devices: s.devices ?? 0, round: s.round, rounds: s.rounds };
  const key = {
    recording: "macros.recording",
    saved: "macros.saved",
    empty: "macros.nothing",
    discarded: "macros.discarded",
    playing: s.rounds ? "macros.playingOf" : "macros.playing",
    finished: "macros.finished",
    stopped: "macros.stopped",
  }[s.phase];
  el.textContent = key ? t(key, vars) : s.phase === "error" ? t("status.error", { error: s.error ?? "" }) : "";
  el.classList.toggle("recording", recording);
}

tuxhub.on("macro-status", (s) => {
  macroStatus = s;
  recording = !!s.recording;
  playing = s.playing ?? null;
  renderMacroStatus();
  renderMacros();
});
tuxhub.on("macros-changed", loadMacros);

// ------------------------------------------------------------------ Editor

let draft = null;

function openEditor(m) {
  draft = structuredClone(m);
  $("ed-title").textContent = m.id ? t("editor.titleEdit") : t("editor.titleNew");
  $("ed-name").value = m.name;
  $("ed-repeat").value = m.repeat;
  $("ed-speed").value = m.speed;
  $("ed-shortcut").value = m.shortcut ?? "";
  $("ed-error").textContent = "";
  $("list-view").classList.add("hidden");
  $("editor").classList.remove("hidden");
  renderSteps();
}

function closeEditor() {
  draft = null;
  $("editor").classList.add("hidden");
  $("list-view").classList.remove("hidden");
}

function describe(s) {
  switch (s.type) {
    case "key":
      return esc(t(s.down ? "step.press" : "step.release", { key: keyLabel(s.code) }));
    case "move":
      return esc(t("step.move", { dx: s.dx, dy: s.dy }));
    case "scroll":
      if (s.dy > 0) return esc(t("step.scrollUp", { n: s.dy }));
      if (s.dy < 0) return esc(t("step.scrollDown", { n: -s.dy }));
      return esc(t("step.scrollSide", { n: s.dx }));
    case "wait":
      return `<span class="muted">${esc(t("step.wait"))}</span>`;
  }
  return esc(s.type);
}

function iconButton(icon, label, data, disabled, variant = "neutral") {
  return `<wa-button size="s" appearance="plain" variant="${variant}" ${data} title="${esc(label)}" ${disabled ? "disabled" : ""}>
    <wa-icon name="${icon}" library="system" variant="solid" label="${esc(label)}"></wa-icon></wa-button>`;
}

function renderSteps() {
  const last = draft.steps.length - 1;
  $("step-rows").innerHTML = draft.steps
    .map(
      (s, i) => `<tr>
        <td class="muted">${i + 1}</td>
        <td class="num"><wa-input size="s" type="number" without-spin-buttons min="0" max="3600000" value="${s.delay_ms}" data-delay="${i}" aria-label="${esc(t("editor.delay"))}"></wa-input></td>
        <td>${describe(s)}</td>
        <td class="num actions">
          ${iconButton("arrow-up", t("editor.moveUp"), `data-up="${i}"`, i === 0)}
          ${iconButton("arrow-down", t("editor.moveDown"), `data-down="${i}"`, i === last)}
          ${iconButton("xmark", t("editor.remove"), `data-rm="${i}"`, false, "danger")}
        </td></tr>`,
    )
    .join("");
  const total = draft.steps.reduce((a, s) => a + (s.delay_ms || 0), 0);
  $("ed-summary").textContent = t("editor.summary", { steps: draft.steps.length, duration: fmtMs(total) });
}

$("step-rows").oninput = (e) => {
  const i = e.target.dataset.delay;
  if (i != null) draft.steps[i].delay_ms = Math.max(0, Math.round(Number(e.target.value) || 0));
};
$("step-rows").onchange = () => renderSteps();
$("step-rows").onclick = (e) => {
  const b = e.target.closest("wa-button");
  if (!b) return;
  const s = draft.steps;
  const swap = (a, c) => ([s[a], s[c]] = [s[c], s[a]]);
  if (b.dataset.up) swap(+b.dataset.up, +b.dataset.up - 1);
  if (b.dataset.down) swap(+b.dataset.down, +b.dataset.down + 1);
  if (b.dataset.rm) s.splice(+b.dataset.rm, 1);
  renderSteps();
};

for (const b of document.querySelectorAll("[data-add]")) {
  b.onclick = () => {
    const code = Number($("add-key").value);
    const add = (step) => draft.steps.push(step);
    if (b.dataset.add === "tap") {
      add({ type: "key", code, down: true, delay_ms: 50 });
      add({ type: "key", code, down: false, delay_ms: 30 });
    }
    if (b.dataset.add === "down") add({ type: "key", code, down: true, delay_ms: 50 });
    if (b.dataset.add === "up") add({ type: "key", code, down: false, delay_ms: 50 });
    if (b.dataset.add === "wait") add({ type: "wait", delay_ms: 500 });
    renderSteps();
    $("step-rows").closest(".steps").scrollTop = 1e9;
  };
}

$("ed-scale").onclick = async () => {
  const answer = await ask({ title: t("editor.scale"), text: t("editor.scalePrompt"), ok: t("editor.apply"), value: "1" });
  const f = Number(answer);
  if (!answer || !f || f <= 0) return;
  for (const s of draft.steps) s.delay_ms = Math.round(s.delay_ms * f);
  renderSteps();
};
$("ed-capture").onclick = async () => {
  const trigger = await captureKey($("ed-capture"));
  if (trigger) $("ed-shortcut").value = trigger;
};
$("ed-cancel").onclick = closeEditor;
$("ed-save").onclick = async () => {
  draft.name = $("ed-name").value;
  draft.repeat = Math.max(0, Math.round(Number($("ed-repeat").value) || 0));
  draft.speed = Number($("ed-speed").value) || 1;
  draft.shortcut = $("ed-shortcut").value.trim() || null;
  try {
    await tuxhub.call("macros.save", draft);
    closeEditor();
  } catch (e) {
    $("ed-error").textContent = e.message;
  }
};

if (tuxhub.standalone) {
  document.documentElement.dataset.standalone = "";
  // Global hotkeys come from tuxhub.
  $("clicker-hotkeys").dataset.i18n = "clicker.hotkeysStandalone";
}
applyStaticTexts();
loadMacros();
