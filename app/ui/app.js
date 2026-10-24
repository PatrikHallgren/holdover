const CYCLES = {
  length: ["m", "yd", "ft", "cm", "mm", "in"],
  mass: ["g", "gr", "kg", "lb", "oz"],
  velocity: ["mps", "fps", "mph", "km/h", "kn"],
  temperature: ["C", "F", "K"],
  pressure: ["hPa", "inHg", "mmHg", "psi", "mbar"],
  angle: ["mrad", "MOA", "IPHY", "cm@100m", "deg"],
  energy: ["J", "ftlb"],
};
const ANGULAR = ["mrad", "MOA", "IPHY", "cm@100m", "clicks"];
const PRESETS = ["metric", "us", "custom"];

let boot = null;
let page = "solve";
let theme = {};
let lastSolve = null;
let stale = false;
let solveError = "";
let fieldMode = false;
let solveTab = "air";
let paletteOpen = false;
let unitsOpen = false;
let equipKind = "rifle";
let selectedId = "";
let equipDraft = false;
let restoreDoc = null;
let restorePreview = null;
let syncTab = "export";
let syncFilter = "all";
let bundlePreview = null;
let bundleBytes = "";
let bundleChoices = {};
let bundleMessage = "";
let bundleDetail = "";
let bundleError = false;
let bundleInclude = false;
let card = null;
let chartData = null;
let chartCursor = null;
let chartPaintWidth = 0;
let chartGeom = {};
let chartObserver = null;
let chartPainting = false;
let solveTimer = 0;
let equipNotice = "";
let moving = { preset: "", speedText: "", speedUnit: "", direction: "3oc", reaction: "0", open: false };
let cardPageId = "a4";
let cardPageTouched = false;
let pbrSpan = "zone";
let pbrWindow = null;
let cardConditions = "";
let cardBrackets = "2 mps, 4 mps, 6 mps";
let chartKinds = { drop: true, wind: true, velocity: false, energy: false, tof: false, mach: false };
let chartPane = "trajectory";
let reticlePane = "view";
let paletteQuery = "";
let paletteIndex = 0;
let pbrResult = null;
let pbrPreview = null;
let pbrZonePreset = "moose";
let pbrMode = "optimal";
let pbrZeroText = "100 m";
let truingKind = "mv";
let truingPreview = null;
let truingForm = { range: "400 m", elevation: "", shots: "", group: "", lines: "", advertised: "10 mrad", observed: "", piecewise: false };
let impactData = null;
let impactTarget = "sjf-algfigur";
let impactGroupId = "";
let groupImpactPending = false;
let groupView = null;
let groupObserver = null;
let groupTimer = 0;
let impactFacing = "left";
let impactRinging = "primary";
let impactSilhouette = true;
let leadTarget = "sjf-algfigur";
let leadFacing = "left";
let reticleTimer = 0;
let reticleScene = null;
let reticleNotice = "";
let targetNotice = "";
let targetEdit = "";
let reticleState = {
  id: "generic-mil-dot",
  mag: 10,
  fov: 22,
  focal: "sfp",
  cal: 10,
  target: "sjf-algfigur",
  rangeText: "100 m",
  facing: "left",
  ringing: "primary",
  snap: 0.5,
  tool: "line",
  showHold: true,
  showSilhouette: true,
  draft: null,
  lineStart: null,
  page: "a4",
};
let chronoResult = null;
let chronoForm = { shots: "", temperature: "15 C", label: "Chronograph", loadId: "", confirmSave: false, confirmMv: false };
let weatherOffer = null;
let weatherMessage = "";
let weatherKind = "";
let weatherBusy = false;
let weatherTimer = 0;
let recoilForm = null;
let recoilResult = null;
let compareIds = [];
let compareReady = false;
let compareEffects = null;
let compareForm = { rangeSi: null, windSi: null, direction: "9oc" };
let compareResult = null;
let compareError = "";
let compareTimer = 0;
let docId = "";
let docNotice = "";
const DASH = ["none", "7 4", "2 3", "9 3 2 3", "12 4", "1 3"];

function t(key) {
  return (boot && boot.strings && boot.strings[key]) || key;
}

function tf(key, vars) {
  let text = t(key);
  const data = vars || {};
  text = text.replace(/\{\s*\$(\w+)\s*->\s*\[one\]\s*([\s\S]*?)\s*\*\[other\]\s*([\s\S]*?)\s*\}/g, (_, name, one, other) => {
    const count = Number(data[name]);
    return Math.abs(count) === 1 ? one : other;
  });
  return text.replace(/\{\s*\$(\w+)\s*\}/g, (_, name) => (data[name] == null ? "" : String(data[name])));
}

function shownUnit(unit) {
  const raw = String(unit || "");
  const key = raw.toLowerCase();
  if (key === "mps") return "m/s";
  if (key === "c") return "°C";
  if (key === "f") return "°F";
  return raw;
}

function esc(value) {
  return String(value ?? "").replace(/[&<>"']/g, (ch) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[ch]);
}

async function call(command, args) {
  if (!window.__TAURI__ || !window.__TAURI__.core) {
    throw new Error("This window is not running inside Holdover.");
  }
  // Argument names are camelCase. Tauri 2 renames bytes_base64 to bytesBase64.
  // Fields inside body stay snake_case; that object is the library JSON.
  return window.__TAURI__.core.invoke(command, args || {});
}

function units() {
  return boot.settings.units;
}

function listenTheme() {
  const core = window.__TAURI__ && window.__TAURI__.event;
  if (!core) return;
  core.listen("theme", (event) => {
    theme = event.payload || {};
    applyTheme();
  });
}

function applyTheme() {
  const root = document.documentElement;
  const map = {
    "--background": theme.background,
    "--surface": theme.surface,
    "--text": theme.text,
    "--text-secondary": theme.text_secondary,
    "--border": theme.border,
    "--accent": theme.accent,
    "--focus": theme.focus || theme.accent,
    "--warning": theme.warning,
    "--error": theme.error,
    "--text-scale": theme.text_scale || 1,
  };
  Object.entries(map).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== "") root.style.setProperty(key, value);
  });
  const derived = {
    "--theme-raised": theme.raised,
    "--theme-rail": theme.rail,
    "--theme-ok": theme.ok,
    "--theme-accent-text": theme.accent_text,
    "--theme-control-border": theme.control_border,
  };
  Object.entries(derived).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== "") root.style.setProperty(key, value);
  });
  (theme.chart || []).forEach((color, index) => root.style.setProperty(`--chart-${index}`, color));
  root.dataset.mode = theme.mode === "light" ? "light" : "dark";
  root.style.colorScheme = theme.mode === "light" ? "light" : "dark";
  document.body.classList.toggle("large-text", !!(boot && boot.settings.large_text));
  document.body.classList.toggle("reduced", !!(boot && boot.settings.reduced_motion));
  document.body.classList.toggle("field", fieldMode);
}

async function refresh() {
  boot = await call("bootstrap");
  if (!cardPageTouched && boot.paper) cardPageId = boot.paper;
  document.title = `${t("app-name")} ${boot.version || ""}`.trim();
  applyTheme();
  render();
}

function render() {
  window.clearInterval(weatherTimer);
  const app = document.getElementById("app");
  const onboarded = boot.settings.onboarded;
  document.body.classList.add("ho");
  document.body.classList.toggle("fit-solve", page === "solve" && onboarded);
  if (!onboarded) {
    app.innerHTML = onboardHtml();
    bindOnboard();
    return;
  }
  const shell = page === "solve" ? solveShell() : pageHtml();
  app.innerHTML = `${shell}${overlays()}`;
  bindShell();
  bindPage();
  if (page === "solve") scheduleSolve(0);
  if (page === "compare") scheduleCompare(0);
}

function navKey(id) {
  const keys = { solve: "nav-solve", card: "nav-card", charts: "nav-charts", reticle: "nav-reticle", targets: "nav-targets", groups: "nav-groups", pbr: "nav-pbr", truing: "nav-truing", equipment: "nav-equipment", compare: "nav-compare", documents: "nav-documents", sync: "nav-sync", settings: "nav-settings", about: "nav-about" };
  return keys[id] || "nav-solve";
}

function topBar() {
  const pages = ["solve", "card", "charts", "reticle", "targets", "groups", "pbr", "truing", "equipment", "compare", "documents", "sync", "settings"];
  return `<a class="ho-skip" href="#inputs">${esc(t("skip"))}</a>
    <header class="ho-top">
      <span class="brand">${esc(t("app-name"))}</span>
      <nav class="ho-nav" aria-label="${esc(t("nav-label"))}">${pages.map(navButton).join("")}</nav>
      <button type="button" class="ho-btn ho-screens" aria-haspopup="dialog" title="${esc(t("screens"))}">${esc(t(navKey(page)))} ▾</button>
      <div class="ho-seg preset" role="radiogroup" aria-label="${esc(t("preset-label"))}">
        ${presetButton("metric")}${presetButton("us")}${presetButton("custom")}
      </div>
      <button type="button" class="ho-btn ghost" id="units-open" aria-expanded="${unitsOpen}" title="${esc(t("units-button"))}">${esc(t("units-button"))}</button>
    </header>`;
}

function keyBit(keys, label, extra) {
  return `<span${extra ? ` class="extra"` : ""}>${keys.map((key) => `<kbd>${esc(key)}</kbd>`).join("")} ${esc(label)}</span>`;
}

function globalKeys() {
  return [
    keyBit(["Ctrl", "K"], t("status-commands")),
    keyBit(["Ctrl", "U"], t("status-preset"), true),
    keyBit(["Ctrl", "P"], t("status-profile"), true),
    keyBit(["F11"], t("status-field"), true),
  ].join("");
}

function pageKeys() {
  const bits = [];
  const rangeStep = page === "card" || page === "reticle" || page === "compare" || page === "truing" || (page === "pbr" && pbrMode === "fixed");
  if (rangeStep) bits.push(keyBit(["↑", "↓"], t("status-range-step")));
  if (page === "compare") {
    bits.push(keyBit(["←", "→"], t("status-clock")));
    bits.push(keyBit(["+", "−"], t("status-wind-step")));
  }
  if (page === "charts") bits.push(keyBit(["←", "→"], t("status-cursor")));
  return bits.join("") + globalKeys();
}

function statusFooter(keys) {
  return `<footer class="ho-status">
    <div class="keys">${keys}</div>
    <span class="sp"></span>
    <span class="ho-legal" title="${esc(boot.disclaimer || "")}">${esc(boot.disclaimer || "")}</span>
    <button type="button" class="ho-btn ghost" id="open-about">${esc(t("nav-about"))}</button>
  </footer>`;
}

function emptyCta(messageKey) {
  return `<p class="ho-hint">${esc(t(messageKey))}</p><p><button type="button" class="ho-btn primary" data-go="equipment">${esc(t("empty-open-equipment"))}</button></p>`;
}

function localDate(iso) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return String(iso || "");
  return date.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function editionDate() {
  return new Date().toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function prettyPhrase(text) {
  return String(text || "")
    .replace(/\bmps\b/g, "m/s")
    .replace(/(^|\s)C\b/g, "$1°C")
    .replace(/(^|\s)F\b/g, "$1°F");
}

function fixedDisplay(text, digits) {
  const number = Number(String(text ?? "").replace(",", "."));
  if (!Number.isFinite(number)) return text == null || text === "" ? "—" : String(text);
  return Math.abs(number).toFixed(digits);
}

function frame(opts) {
  const bodyClass = opts.body ? ` ${opts.body}` : "";
  const rail = opts.rail
    ? `<aside class="ho-rail" id="inputs" tabindex="-1" aria-label="${esc(opts.railLabel || t("inputs"))}">${opts.rail}</aside>`
    : "";
  const mainId = opts.rail ? "main" : "inputs";
  return `<div class="ho-shell">${topBar()}<div class="ho-body${bodyClass}">${rail}<main class="ho-main" id="${mainId}" tabindex="-1">${opts.main}</main></div>${statusFooter(opts.keys || pageKeys())}</div>`;
}

function overlays() {
  return `<div id="palette" class="ho-scrim${paletteOpen ? "" : " hidden"}" role="dialog" aria-modal="true" aria-label="${esc(t("commands"))}"></div>
    <div id="units-pop" class="ho-scrim${unitsOpen ? "" : " hidden"}" role="dialog" aria-modal="true" aria-label="${esc(t("units-button"))}">
      <div class="ho-dialog"><header><h2>${esc(t("settings-units"))}</h2></header><div class="body">${unitsPopover()}</div></div>
    </div>`;
}

function unitsPopover(includeApply = true) {
  const u = units();
  const row = (key, label, cycle) => `<div class="ho-field"><div class="head"><label for="unit-${key}">${esc(t(label))}</label></div>
    <div class="ho-entry"><select id="unit-${key}" data-unit-key="${key}">${cycle.map((item) => `<option value="${esc(item)}" ${item === u[key] ? "selected" : ""}>${esc(shownUnit(item))}</option>`).join("")}</select></div>
  </div>`;
  return `<div class="ho-fields">
    ${row("length", "custom-length", CYCLES.length)}
    ${row("small", "custom-small", CYCLES.length)}
    ${row("angle", "custom-angle", CYCLES.angle)}
    ${row("temperature", "custom-temperature", CYCLES.temperature)}
    ${row("pressure", "custom-pressure", CYCLES.pressure)}
    ${row("velocity", "custom-velocity", CYCLES.velocity)}
    ${row("mass", "custom-mass", CYCLES.mass)}
    ${row("energy", "custom-energy", CYCLES.energy)}
    ${row("angular", "custom-angular", ANGULAR)}
    ${includeApply ? `<button type="button" class="ho-btn primary" id="units-apply">${esc(t("apply"))}</button>` : ""}
  </div>`;
}

function settingsUnitFields() {
  const u = units();
  const row = (key, label, cycle) => `<div class="ho-field"><div class="head"><label for="settings-unit-${key}">${esc(t(label))}</label></div>
    <div class="ho-entry"><select id="settings-unit-${key}" data-settings-unit="${key}">${cycle.map((item) => `<option value="${esc(item)}" ${item === u[key] ? "selected" : ""}>${esc(shownUnit(item))}</option>`).join("")}</select></div>
  </div>`;
  return [
    row("length", "custom-length", CYCLES.length),
    row("small", "custom-small", CYCLES.length),
    row("angle", "custom-angle", CYCLES.angle),
    row("temperature", "custom-temperature", CYCLES.temperature),
    row("pressure", "custom-pressure", CYCLES.pressure),
    row("velocity", "custom-velocity", CYCLES.velocity),
    row("mass", "custom-mass", CYCLES.mass),
    row("energy", "custom-energy", CYCLES.energy),
    row("angular", "custom-angular", ANGULAR),
  ].join("");
}

function pageHtml() {
  if (page === "card") return cardHtml();
  if (page === "charts") return chartHtml();
  if (page === "reticle") return reticleHtml();
  if (page === "targets") return targetsHtml();
  if (page === "groups") return groupsHtml();
  if (page === "pbr") return pbrHtml();
  if (page === "truing") return truingHtml();
  if (page === "equipment") return equipmentHtml();
  if (page === "compare") return compareHtml();
  if (page === "documents") return documentsHtml();
  if (page === "sync") return syncHtml();
  if (page === "about") return aboutHtml();
  if (page === "settings") return settingsHtml();
  return "";
}

function weatherBlock() {
  return `<p id="air-badge" class="badge-fresh"></p>
    <p id="air-detail" class="meta"></p>
    <div class="row">
      <button type="button" id="weather-fetch" title="${esc(t("weather-fetch-tip"))}" ${weatherBusy ? "disabled" : ""}>${esc(t("weather-fetch"))}</button>
    </div>
    <p class="meta">${boot.settings.weather_place ? `${esc(t("weather-place-using"))} ${esc(boot.settings.weather_place)}` : esc(t("weather-no-place"))}</p>
    <p id="weather-status" class="${weatherKind && weatherKind !== "place" ? "weather-error" : "meta"}">${esc(weatherMessage)}</p>
    ${weatherOffer ? weatherResultHtml(weatherOffer) : ""}
    <p class="hint">${esc(t("weather-attribution"))}</p>
    <details class="ho-details">
      <summary>${esc(t("weather-about"))}</summary>
      <p class="hint">${esc(t("weather-intro"))}</p>
      <p class="hint">${esc(t("weather-noncommercial"))}</p>
    </details>`;
}

function weatherResultHtml(offer) {
  const u = units();
  const temp = formatLocal(offer.temperature_c, "temperature", u.temperature);
  const station = formatLocal(offer.surface_pressure_pa, "pressure", u.pressure);
  const sea = formatLocal(offer.sea_level_pressure_pa, "pressure", u.pressure);
  const elev = formatLocal(offer.elevation_m, "length", u.length);
  const wind = offer.wind_speed_mps == null ? "—" : `${formatLocal(offer.wind_speed_mps, "velocity", u.velocity)} ${shownUnit(u.velocity)}`;
  const gust = offer.wind_gust_mps == null ? "—" : `${formatLocal(offer.wind_gust_mps, "velocity", u.velocity)} ${shownUnit(u.velocity)}`;
  const dir = offer.wind_from_deg == null ? "—" : String(offer.wind_from_deg);
  return `<div class="fields">
    <p class="meta">${esc(t("weather-fetched"))} ${esc(offer.fetched_at)} · ${esc(t("weather-valid"))} ${esc(offer.valid_time)}</p>
    <p>${esc(t("temperature"))}: ${esc(temp)} ${esc(u.temperature)}</p>
    <p>${esc(t("humidity"))}: ${esc(String(Math.round(offer.humidity * 1000) / 10))}%</p>
    <p>${esc(t("weather-station-used"))}: ${esc(station)} ${esc(u.pressure)}</p>
    <p>${esc(t("weather-sea-context"))}: ${esc(sea)} ${esc(u.pressure)}</p>
    <p>${esc(t("weather-elevation"))}: ${esc(elev)} ${esc(u.length)}</p>
    <p class="meta">${esc(t("weather-grid"))} ${esc(offer.grid_latitude)}, ${esc(offer.grid_longitude)}</p>
    <p>${esc(t("weather-wind-context"))}: ${esc(wind)} · ${esc(t("weather-from"))} ${esc(dir)}° · ${esc(t("weather-gust"))} ${esc(gust)}</p>
    <p class="hint">${esc(t("weather-wind-note"))}</p>
    <div class="row">
      <button type="button" id="weather-use" title="${esc(t("weather-use-tip"))}">${esc(t("weather-use"))}</button>
      <button type="button" id="weather-use-sea" title="${esc(t("weather-use-sea-tip"))}">${esc(t("weather-use-sea"))}</button>
      <button type="button" id="weather-use-wind" title="${esc(t("weather-use-wind-tip"))}">${esc(t("weather-use-wind"))}</button>
      <button type="button" id="weather-dismiss" title="${esc(t("weather-dismiss-tip"))}">${esc(t("weather-dismiss"))}</button>
    </div>
  </div>`;
}

function paintAirBadge() {
  const badge = document.getElementById("air-badge");
  const detail = document.getElementById("air-detail");
  if (!badge) return;
  const air = (boot.settings && boot.settings.current_air) || {};
  const prov = air.provenance || {};
  if (prov.source !== "fetched" || !prov.fetched_at) {
    badge.textContent = t("atmosphere-manual");
    badge.className = "badge-fresh";
    if (detail) detail.textContent = t("weather-manual-note");
    return;
  }
  const age = isoAgeSeconds(prov.fetched_at);
  const limit = Number(boot.settings.weather_stale_minutes || 60) * 60;
  const stale = Number.isFinite(age) && age > limit;
  const kind = prov.pressure_used === "sea-level" ? t("pressure-sea") : t("weather-station-label");
  const elev = formatLocal(Number(prov.elevation_m), "length", units().length);
  badge.textContent = `${stale ? t("weather-stale") : t("weather-fetched")} · ${agePhrase(age)}`;
  badge.className = stale ? "badge-stale" : "badge-fresh";
  if (detail) {
    const sea = prov.sea_level_pressure_pa == null ? "" : `${t("weather-sea-context")}: ${formatLocal(Number(prov.sea_level_pressure_pa), "pressure", units().pressure)} ${units().pressure}. `;
    detail.textContent = `${t("weather-valid")} ${prov.valid_time || "—"}. ${kind}. ${t("weather-elevation")} ${elev} ${units().length}. ${sea}${t("weather-source")}: Open-Meteo.`;
  }
}

function agePhrase(seconds) {
  if (!Number.isFinite(seconds) || seconds < 60) return t("weather-age-now");
  if (seconds < 3600) return t("weather-age-min").replace("{n}", String(Math.floor(seconds / 60)));
  if (seconds < 86400) return t("weather-age-h").replace("{n}", String(Math.floor(seconds / 3600)));
  return t("weather-age-d").replace("{n}", String(Math.floor(seconds / 86400)));
}

function isoAgeSeconds(text) {
  const match = String(text).match(/^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z$/);
  if (!match) return NaN;
  const utc = Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3]), Number(match[4]), Number(match[5]), Number(match[6]));
  return Math.max(0, Math.floor((Date.now() - utc) / 1000));
}

function airFieldsDiffer() {
  const air = boot.settings.current_air || {};
  const compare = (id, dim, node) => {
    const input = document.getElementById(id);
    if (!input || !node || node.si == null) return false;
    const parsed = parseLocal(input.value, dim, input.dataset.unit);
    if (!parsed) return true;
    const limit = dim === "pressure" ? 50 : dim === "length" ? 0.5 : 0.05;
    return Math.abs(parsed.si - Number(node.si)) > limit;
  };
  if (compare("temperature", "temperature", air.temperature)) return true;
  if (compare("pressure", "pressure", air.pressure)) return true;
  if (compare("altitude", "length", air.altitude)) return true;
  const humidity = document.getElementById("humidity");
  if (humidity) {
    const stored = typeof air.humidity === "string" ? air.humidity : (air.humidity_text || "");
    if (stored && humidity.value.trim() !== String(stored).trim()) return true;
  }
  const kind = document.getElementById("pressure_kind");
  return !!(kind && kind.value !== (air.pressure_kind || "station"));
}

function noteAirEdit() {
  const air = boot.settings.current_air || {};
  if (!air.provenance || air.provenance.source !== "fetched") return;
  if (!airFieldsDiffer()) return;
  air.provenance = { source: "manual" };
  paintAirBadge();
}

function weatherFailure(kind) {
  const keys = { offline: "weather-offline", timeout: "weather-timeout", http: "weather-http", parse: "weather-parse", place: "weather-no-place" };
  return t(keys[kind] || "weather-offline");
}

function explainImport(error) {
  const text = String(error || "");
  if (text === "not-holdover") return t("import-not-holdover");
  if (text === "schema-newer") return t("import-schema-newer");
  if (text === "schema-older") return t("import-schema-older");
  if (text === "bad-row") return t("import-bad-row");
  if (text.startsWith("missing:")) return t("import-missing").replace("{key}", text.slice("missing:".length));
  if (text.startsWith("duplicate:")) return t("import-duplicate").replace("{id}", text.slice("duplicate:".length));
  if (text === "place-latitude") return t("place-latitude");
  if (text === "place-longitude") return t("place-longitude");
  if (text === "place-stale") return t("place-stale");
  return t("import-failed");
}

function libraryPreviewText(preview) {
  const counts = preview.counts || {};
  const countLine = Object.keys(counts).map((key) => `${key} ${counts[key]}`).join(", ");
  const lines = [
    `${t("import-schema")} ${preview.schema_version}`,
    t("import-nothing-yet"),
    `${t("import-counts")}: ${countLine}`,
    `${t("import-duplicates")}: ${(preview.duplicates || []).length}`,
    `${t("import-local-only")}: ${preview.local_only || 0}`,
    t("import-replace-note"),
    t("import-merge-note"),
  ];
  (preview.duplicates || []).slice(0, 8).forEach((row) => {
    lines.push(`${row.kind} ${row.label || row.id}`);
  });
  return lines.join("\n");
}

async function fetchWeather() {
  weatherBusy = true;
  weatherKind = "";
  weatherMessage = t("weather-fetching");
  const status = document.getElementById("weather-status");
  const button = document.getElementById("weather-fetch");
  if (status) {
    status.className = "meta";
    status.textContent = weatherMessage;
  }
  if (button) button.disabled = true;
  try {
    const result = await call("fetch_weather");
    if (!result || result.ok === false) {
      weatherOffer = null;
      weatherKind = (result && result.kind) || "offline";
      weatherMessage = weatherFailure(weatherKind);
    } else {
      weatherOffer = result;
      weatherKind = "";
      weatherMessage = "";
    }
  } catch (error) {
    weatherOffer = null;
    weatherKind = "offline";
    weatherMessage = weatherFailure("offline");
  }
  weatherBusy = false;
  const host = document.getElementById("weather-block");
  if (host) {
    host.innerHTML = weatherBlock();
    bindWeather();
    paintAirBadge();
  }
}

async function useWeather(pressureUsed) {
  if (!weatherOffer) return;
  const body = Object.assign({}, weatherOffer, {
    pressure_used: pressureUsed,
    place_name: boot.settings.weather_place || "",
  });
  boot = await call("apply_weather", { body });
  render();
}

async function useWind() {
  if (!weatherOffer || weatherOffer.wind_speed_mps == null) return;
  boot = await call("apply_weather", { body: { accept_wind: true, wind_speed_mps: weatherOffer.wind_speed_mps } });
  render();
}

function bindWeather() {
  const fetchButton = document.getElementById("weather-fetch");
  if (fetchButton) fetchButton.addEventListener("click", fetchWeather);
  const use = document.getElementById("weather-use");
  if (use) use.addEventListener("click", () => useWeather("station"));
  const sea = document.getElementById("weather-use-sea");
  if (sea) sea.addEventListener("click", () => useWeather("sea-level"));
  const wind = document.getElementById("weather-use-wind");
  if (wind) wind.addEventListener("click", useWind);
  const dismiss = document.getElementById("weather-dismiss");
  if (dismiss) dismiss.addEventListener("click", () => {
    weatherOffer = null;
    weatherMessage = "";
    weatherKind = "";
    const host = document.getElementById("weather-block");
    if (host) {
      host.innerHTML = weatherBlock();
      bindWeather();
      paintAirBadge();
    }
  });
  paintAirBadge();
  window.clearInterval(weatherTimer);
  weatherTimer = window.setInterval(paintAirBadge, 15000);
}

function presetButton(id) {
  const on = boot.settings.preset === id;
  return `<button type="button" role="radio" data-preset="${id}" aria-checked="${on}" tabindex="${on ? "0" : "-1"}">${esc(t("preset-" + id))}</button>`;
}

function solveTabButton(id, label) {
  const on = solveTab === id;
  return `<button type="button" role="tab" id="solve-tab-${id}" data-tab="${id}" aria-controls="solve-panel-${id}" aria-selected="${on}" tabindex="${on ? "0" : "-1"}">${esc(label)}</button>`;
}

function solvePanel(id, title, meta, body) {
  const wide = window.matchMedia("(min-width: 1400px)").matches;
  const on = solveTab === id || wide;
  return `<div role="tabpanel" class="solve-panel${on ? "" : " is-hidden"}" id="solve-panel-${id}" data-tab="${id}" aria-labelledby="solve-tab-${id}" aria-hidden="${on ? "false" : "true"}">
    <header class="solve-wide-head"><h2>${esc(title)}</h2><span class="meta">${esc(meta || "")}</span></header>
    ${body}
  </div>`;
}

function solveShell() {
  const s = boot.settings;
  const profiles = boot.equipment.profiles || [];
  const options = profiles.map((item) => `<option value="${esc(item.id)}" ${item.id === s.active_profile_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const active = profiles.find((item) => item.id === s.active_profile_id);
  const air = `${airFields(s.current_air || {}, "", true)}
    <div id="weather-block">${weatherBlock()}</div>`;
  const lead = `<div class="ho-row">
      <div class="ho-field">
        <div class="head"><label for="lead-preset">${esc(t("lead-preset"))}</label></div>
        <div class="ho-entry"><select id="lead-preset">
          <option value="">${esc(t("overlay-none"))}</option>
          ${(boot.running || []).map((item) => `<option value="${esc(item.id)}" ${moving.preset === item.id ? "selected" : ""}>${esc(item.name)}</option>`).join("")}
        </select></div>
      </div>
      ${qtyField("target_speed", t("target-speed"), moving.speedText, moving.speedUnit || units().velocity, "velocity", null, null, "target_speed")}
    </div>
    <div class="ho-row ho-lead">
      <div class="ho-field">
        <div class="head"><label for="target_direction">${esc(t("lead-direction"))}</label></div>
        <div class="ho-entry"><input id="target_direction" value="${esc(moving.direction || "3 o'clock")}" autocomplete="off" spellcheck="false" /></div>
        <p class="ho-hint">${esc(t("lead-clock"))}</p>
      </div>
      <div class="ho-field">
        <div class="head"><label for="reaction_s">${esc(t("lead-reaction"))}</label></div>
        <div class="ho-entry"><input id="reaction_s" class="num" value="${esc(moving.reaction || "0")}" inputmode="decimal" autocomplete="off" spellcheck="false" /><span class="ho-chip">s</span></div>
      </div>
    </div>
    <div class="ho-row">
      <div class="ho-field" title="${esc(t("lead-target-tip"))}">
        <div class="head"><label for="lead-target">${esc(t("impact-target"))}</label></div>
        <div class="ho-entry"><select id="lead-target" title="${esc(t("lead-target-tip"))}">
          <option value="">${esc(t("lead-target-none"))}</option>
          ${targetOptions(leadTarget, ["sjf", "svsf"])}
        </select></div>
      </div>
      <div class="ho-field" title="${esc(t("facing-tip"))}">
        <div class="head"><label for="lead-facing">${esc(t("facing"))}</label></div>
        <div class="ho-entry"><select id="lead-facing" title="${esc(t("facing-tip"))}">
          <option value="left" ${leadFacing === "left" ? "selected" : ""}>${esc(t("facing-left"))}</option>
          <option value="right" ${leadFacing === "right" ? "selected" : ""}>${esc(t("facing-right"))}</option>
        </select></div>
      </div>
    </div>
    <p class="ho-hint" id="lead-note">${esc(leadNote())}</p>
    <div id="lead-result">${leadHold()}</div>`;
  const effects = `<div class="checks" id="effects">
      ${check("incline", true)}${check("spin_drift", true)}${check("coriolis", false)}${check("cant", false)}${check("aerodynamic_jump", false)}
    </div>
    <div id="extra" class="hidden">
      <div class="ho-row">
        ${qtyField("latitude", t("latitude"), "0 deg", "deg", "angle")}
        ${qtyField("azimuth", t("azimuth"), "0 deg", "deg", "angle")}
        ${qtyField("cant", t("cant-angle"), "0 deg", "deg", "angle")}
      </div>
    </div>
    <p class="ho-hint">${esc(t("solve-tab-effects-intro"))}</p>
    <p class="ho-hint">${esc(t("field-suffix-hint"))}</p>
    <p class="ho-hint">${esc((boot && boot.spin_provenance) || t("spin-note"))}</p>
    <p class="ho-hint">${esc(t("jump-note"))}</p>`;
  const angular = s.angular || "mrad";
  return `<div class="ho-shell">
    ${topBar()}
    <div class="ho-body">
      <aside class="ho-rail" id="inputs" tabindex="-1" aria-label="${esc(t("shot"))}">
        <div class="ho-summary">
          <div class="head">
            <select id="profile" aria-label="${esc(t("profile"))}">${options || `<option value="">${esc(t("no-profile"))}</option>`}</select>
            <span class="ho-keys"><kbd>Ctrl</kbd> <kbd>P</kbd></span>
          </div>
          <dl class="ho-kv mini" id="profile-facts">${profileFacts(active)}</dl>
        </div>
        <p class="banner hidden" id="differs">${esc(t("air-differs"))}</p>
        <h2>${esc(t("shot"))}</h2>
        ${qtyField("range", t("range"), s.last_range, s.last_range_unit || units().length, "length", s.last_range_si, s.last_range_display, "range")}
        ${qtyField("wind_speed", t("wind-speed"), s.last_wind_speed, s.last_wind_speed_unit || units().velocity, "velocity", s.last_wind_speed_si, s.last_wind_speed_display, "wind_speed")}
        ${windClockField(s.last_wind_direction || "12oc")}
        ${qtyField("inclination", t("inclination"), s.last_inclination, "deg", "angle", s.last_inclination_si, s.last_inclination_display, "inclination")}
      </aside>
      <main class="ho-main" id="main" tabindex="-1">
        <section class="ho-hero" id="hold" aria-label="${esc(t("hold"))}" aria-live="polite">
          <div class="ho-readout${stale ? " ho-stale" : ""}" id="hold-elev"></div>
          <div class="ho-readout${stale ? " ho-stale" : ""}" id="hold-wind"></div>
          <div class="ho-context">
            <div class="bar">
              <span class="ho-cap">${esc(t("hold-unit"))} <kbd>U</kbd></span>
              <div class="ho-seg small" role="radiogroup" aria-label="${esc(t("hold-unit"))}">
                ${ANGULAR.map((item) => `<button type="button" role="radio" data-angular="${esc(item)}" aria-checked="${angular === item}" tabindex="${angular === item ? "0" : "-1"}">${esc(t(angularKey(item)))}</button>`).join("")}
              </div>
            </div>
            <dl class="ho-ctx" id="hold-ctx"></dl>
            <button type="button" class="ho-btn" id="copy-hold" title="${esc(t("copy-hold"))}"><span>${esc(t("copy-hold"))}</span> <kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>C</kbd></button>
          </div>
          <div class="ho-reason-row" id="solve-error" ${stale ? "" : "hidden"}></div>
        </section>
        <div class="ho-cards">
          <section class="ho-card${stale ? " ho-stale" : ""}" id="traj-card">
            <header><h2>${esc(t("trajectory"))}</h2><span class="meta" id="traj-meta"></span></header>
            <dl class="ho-kv" id="traj-body"></dl>
          </section>
          <section class="ho-card${stale ? " ho-stale" : ""}" id="sens-card">
            <header><h2>${esc(t("sensitivity"))}</h2><span class="meta">${esc(t("vs-now"))}</span></header>
            <div id="sens-body"></div>
          </section>
          <section class="ho-card solve-tabs">
            <div class="ho-tabs" role="tablist" aria-label="${esc(t("solve-tabs"))}">
              ${solveTabButton("air", t("solve-tab-air"))}
              ${solveTabButton("lead", t("solve-tab-lead"))}
              ${solveTabButton("effects", t("solve-tab-effects"))}
            </div>
            ${solvePanel("air", t("solve-tab-air"), t("solve-air-meta"), air)}
            ${solvePanel("lead", t("solve-tab-lead"), t("lead"), lead)}
            ${solvePanel("effects", t("solve-tab-effects"), "", effects)}
          </section>
        </div>
        <ul class="ho-notes" id="solve-notes" aria-label="${esc(t("notes"))}"></ul>
      </main>
    </div>
    ${statusFooter(statusKeys())}
  </div>`;
}

function navButton(id) {
  const keys = { solve: "nav-solve", card: "nav-card", charts: "nav-charts", reticle: "nav-reticle", targets: "nav-targets", groups: "nav-groups", pbr: "nav-pbr", truing: "nav-truing", equipment: "nav-equipment", compare: "nav-compare", documents: "nav-documents", sync: "nav-sync", settings: "nav-settings" };
  const current = page === id ? ` aria-current="page"` : "";
  return `<button type="button" data-page="${id}"${current}>${esc(t(keys[id] || "nav-solve"))}</button>`;
}

function angularKey(item) {
  return { mrad: "angular-mrad", MOA: "angular-moa", IPHY: "angular-iphy", "cm@100m": "angular-cm", clicks: "angular-clicks" }[item] || "angular-mrad";
}

function statusKeys() {
  return [
    keyBit(["R"], t("status-range")),
    keyBit(["W"], t("status-wind")),
    keyBit(["D"], t("status-direction")),
    keyBit(["A"], t("status-angle")),
    keyBit(["↑", "↓"], t("status-range-step")),
    keyBit(["←", "→"], t("status-clock")),
    keyBit(["+", "−"], t("status-wind-step")),
    keyBit(["U"], t("status-hold-unit")),
    keyBit(["Ctrl", "Shift", "C"], t("status-copy")),
    globalKeys(),
  ].join("");
}

function profileFacts(profile) {
  if (!profile) return `<dt>${esc(t("profile"))}</dt><dd>${esc(t("no-profile"))}</dd><span class="ho-unit"></span>`;
  const air = profile.zero_air || {};
  const temp = air.temperature_display ? `${air.temperature_display} ${shownUnit(air.temperature_unit || "")}` : "";
  const scope = (boot.equipment.scopes || []).find((item) => item.id === profile.scope_id);
  const truing = (lastSolve && lastSolve.truing) || t("truing-none");
  const warn = !lastSolve || !lastSolve.truing;
  const truingHtml = warn ? `<span class="ho-badge warn">${esc(String(truing))}</span>` : esc(String(truing));
  return `<dt>${esc(t("zero-range"))}</dt><dd>${esc(profile.zero_range_display || "")}</dd><span class="ho-unit">${esc(shownUnit(profile.zero_range_unit || ""))}</span>
    <dt>${esc(t("zero-air"))}</dt><dd>${esc(temp)}</dd><span class="ho-unit"></span>
    <dt>${esc(t("click"))}</dt><dd>${esc(scope && scope.click_display || "")}</dd><span class="ho-unit">${esc(shownUnit(scope && scope.click_unit || ""))}</span>
    <dt>${esc(t("truing"))}</dt><dd id="truing-line">${truingHtml}</dd><span class="ho-unit"></span>`;
}

function windClockField(value, opts) {
  const options = opts || {};
  const id = options.id || "wind_direction";
  const stepper = options.stepper || "wind_direction";
  const label = options.label || t("wind-direction");
  const hour = clockHour(value);
  const buttons = [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].map((item) =>
    `<button type="button" role="radio" data-hour="${item}" aria-checked="${item === hour}" title="${esc(tf("clock-hour", { hour: item }))}">${item}</button>`).join("");
  const hint = tf("hint-clock", { rest: t("wind-clock-hint") });
  const key = options.key === false ? "" : `<kbd>D</kbd>`;
  const minusLabel = tf("step-minus", { field: label, step: t("one-hour") });
  const plusLabel = tf("step-plus", { field: label, step: t("one-hour") });
  return `<div class="ho-field" data-stepper="${esc(stepper)}">
    <div class="head"><label for="${id}">${esc(label)}</label>${key}</div>
    <div class="ho-stepper">
      <button type="button" class="ho-step" data-dir="-1" tabindex="-1" aria-label="${esc(minusLabel)}" title="${esc(minusLabel)}">−</button>
      <input id="${id}" value="${esc(clockPhrase(value))}" title="${esc(t("wind-clock-hint"))}" role="spinbutton" autocomplete="off" spellcheck="false" />
      <button type="button" class="ho-step" data-dir="1" tabindex="-1" aria-label="${esc(plusLabel)}" title="${esc(plusLabel)}">+</button>
    </div>
    <div class="ho-clock" role="radiogroup" aria-label="${esc(t("wind-clock-hint"))}">${buttons}</div>
    <p class="ho-hint" data-base="${esc(hint)}">${esc(hint)}</p>
  </div>`;
}

function clockHour(value) {
  const match = String(value || "").match(/(\d+)/);
  let hour = match ? Number(match[1]) : 12;
  hour = ((hour % 12) + 12) % 12;
  return hour === 0 ? 12 : hour;
}

function clockPhrase(value) {
  const text = String(value || "").trim();
  if (/deg/i.test(text)) return text;
  return tf("clock-hour", { hour: clockHour(text) });
}

function clockOk(text) {
  const clock = /^\s*\d{1,2}\s*(?:oc|o['’]?clock)?\s*$/i;
  const degrees = /deg/i;
  const value = String(text || "");
  return clock.test(value) || degrees.test(value);
}

function check(name, fallback) {
  const flags = boot.settings.effects || {};
  const on = flags[name] !== undefined ? flags[name] : fallback;
  const key = name === "spin_drift" ? "effect-spin" : name === "aerodynamic_jump" ? "effect-jump" : "effect-" + name;
  return `<label class="ho-check"><input type="checkbox" data-effect="${name}" ${on ? "checked" : ""} /> ${esc(t(key))}</label>`;
}

function airPart(air, name, fallbackUnit) {
  const node = air && air[name];
  if (node && typeof node === "object") {
    return {
      text: node.text || "",
      unit: node.display_unit || fallbackUnit,
      si: node.si,
      display: node.display,
    };
  }
  const suffix = name === "temperature" ? "_c" : name === "pressure" ? "_pa" : "_m";
  return {
    text: (air && air[name + "_text"]) || "",
    unit: (air && air[name + "_unit"]) || fallbackUnit,
    si: air && air[name + suffix],
    display: air && air[name + "_display"],
  };
}

function airFields(air, prefix, stepped) {
  const temperature = airPart(air, "temperature", units().temperature);
  const pressure = airPart(air, "pressure", units().pressure);
  const altitude = airPart(air, "altitude", units().length);
  const humidity = air && typeof air.humidity === "string" ? air.humidity : ((air && air.humidity_text) || "0%");
  const kind = `<select id="${prefix}pressure_kind">
      <option value="station" ${!air || air.pressure_kind !== "sea-level" ? "selected" : ""}>${esc(t("pressure-station"))}</option>
      <option value="sea-level" ${air && air.pressure_kind === "sea-level" ? "selected" : ""}>${esc(t("pressure-sea"))}</option>
    </select>`;
  if (!stepped) {
    return `<div class="ho-row ho-grid">
      ${qtyField(prefix + "temperature", t("temperature"), temperature.text, temperature.unit, "temperature", temperature.si, temperature.display)}
      ${qtyField(prefix + "pressure", t("pressure"), pressure.text, pressure.unit, "pressure", pressure.si, pressure.display)}
    </div>
    <div class="ho-row ho-grid">
      <div class="ho-field">
        <div class="head"><label for="${prefix}pressure_kind">${esc(t("kind"))}</label></div>
        <div class="ho-entry">${kind}</div>
      </div>
      <div class="ho-field">
        <div class="head"><label for="${prefix}humidity">${esc(t("humidity"))}</label></div>
        <div class="ho-entry"><input id="${prefix}humidity" value="${esc(humidity)}" autocomplete="off" /></div>
        <p class="ho-hint">${esc(t("humidity-hint"))}</p>
      </div>
      ${qtyField(prefix + "altitude", t("altitude"), altitude.text, altitude.unit, "length", altitude.si, altitude.display)}
    </div>`;
  }
  return `<div class="ho-row ho-grid">
      ${qtyField(prefix + "temperature", t("temperature"), temperature.text, temperature.unit, "temperature", temperature.si, temperature.display, "temperature")}
      ${qtyField(prefix + "pressure", t("pressure"), pressure.text, pressure.unit, "pressure", pressure.si, pressure.display, "pressure")}
    </div>
    <div class="ho-row ho-grid">
      <div class="ho-field">
        <div class="head"><label for="${prefix}pressure_kind">${esc(t("kind"))}</label></div>
        <div class="ho-entry">${kind}</div>
      </div>
      ${humidityField(prefix, humidity)}
      ${qtyField(prefix + "altitude", t("altitude"), altitude.text, altitude.unit, "length", altitude.si, altitude.display)}
    </div>`;
}

function humidityField(prefix, humidity) {
  const id = `${prefix}humidity`;
  const label = t("humidity");
  const amount = "5 %";
  return `<div class="ho-field" data-stepper="humidity">
    <div class="head"><label for="${id}">${esc(label)}</label></div>
    <div class="ho-stepper">
      <button type="button" class="ho-step" data-dir="-1" tabindex="-1" aria-label="${esc(tf("step-minus", { field: label, step: amount }))}">−</button>
      <input id="${id}" data-kind="percent" value="${esc(humidity)}" role="spinbutton" class="num" autocomplete="off" spellcheck="false" />
      <span class="ho-chip">%</span>
      <button type="button" class="ho-step" data-dir="1" tabindex="-1" aria-label="${esc(tf("step-plus", { field: label, step: amount }))}">+</button>
    </div>
    <p class="ho-hint" data-base="${esc(`${tf("hint-step", { step: amount })} · ${t("humidity-hint")}`)}">${esc(`${tf("hint-step", { step: amount })} · ${t("humidity-hint")}`)}</p>
  </div>`;
}

function qtyField(id, label, text, unit, dim, si, display, step) {
  const raw = text == null ? "" : String(text);
  const numeric = Number(si);
  const hasSi = si !== undefined && si !== null && si !== "" && Number.isFinite(numeric);
  const shown = display !== undefined && display !== null && String(display) !== ""
    ? String(display)
    : (hasSi ? formatLocal(numeric, dim, unit) : raw);
  const value = unit ? stripTrailingUnit(shown, unit) : shown;
  const input = `<input id="${id}" data-dim="${dim}" data-unit="${esc(unit)}" data-text="${esc(raw)}" data-si="${hasSi ? esc(String(numeric)) : ""}" data-dirty="0" value="${esc(value)}" autocomplete="off" spellcheck="false" />`;
  return hoQty(id, label, unit, dim, step, input);
}

function hoQty(id, label, unit, dim, step, input) {
  const amount = step ? stepAmount(step, unit) : "";
  const hint = step ? (step === "wind_speed" ? tf("hint-wind", { step: amount }) : tf("hint-step", { step: amount })) : "";
  const key = { range: "R", wind_speed: "W", inclination: "A" }[id] || "";
  const chip = unit ? `<button type="button" class="chip ho-chip" data-for="${id}" title="${esc(label)}">${esc(shownUnit(unit))}</button>` : "";
  const minus = step ? `<button type="button" class="ho-step" data-dir="-1" tabindex="-1" aria-label="${esc(tf("step-minus", { field: label, step: amount }))}" title="${esc(tf("step-minus", { field: label, step: amount }))}">−</button>` : "";
  const plus = step ? `<button type="button" class="ho-step" data-dir="1" tabindex="-1" aria-label="${esc(tf("step-plus", { field: label, step: amount }))}" title="${esc(tf("step-plus", { field: label, step: amount }))}">+</button>` : "";
  const boxed = input.replace("<input ", `<input class="num"${step ? ` role="spinbutton"` : ""} `);
  return `<div class="ho-field"${step ? ` data-stepper="${esc(step)}"` : ""}>
    <div class="head"><label for="${id}">${esc(label)}</label>${key ? `<kbd>${key}</kbd>` : ""}</div>
    <div class="${step ? "ho-stepper" : "ho-entry"}">${minus}${boxed}${chip}${plus}</div>
    <p class="ho-hint" id="${id}-preview" data-base="${esc(hint)}">${esc(hint)}</p>
  </div>`;
}

function stripTrailingUnit(text, unit) {
  const raw = String(text || "").trim();
  const parts = raw.split(/\s+/);
  if (parts.length < 2) return raw;
  const tail = parts[parts.length - 1];
  const aliases = new Set([String(unit || ""), shownUnit(unit), "mps", "m/s", "C", "°C", "F", "°F", "deg", "°"].filter(Boolean));
  if (!aliases.has(tail)) return raw;
  return parts.slice(0, -1).join(" ");
}

function stepSettings() {
  return {
    rangeStepSi: (parseLocal(boot.settings.range_step, "length", "m") || { si: 10 }).si,
    windStepSi: (parseLocal(boot.settings.wind_step, "velocity", "mps") || { si: 1 }).si,
  };
}

function stepAmount(step, unit) {
  const settings = stepSettings();
  const rangeSteps = ["range", "zero_range", "compare-range", "reticle-range", "impact-range", "observe_range", "pbr-zero", "from", "to", "step"];
  if (rangeSteps.includes(step)) return `${formatLocal(settings.rangeStepSi, "length", unit)} ${shownUnit(unit)}`;
  if (step === "wind_speed" || step === "compare-wind") return `${formatLocal(settings.windStepSi, "velocity", unit)} ${shownUnit(unit)}`;
  if (step === "inclination") return `${formatDelta(Math.PI / 180, "angle", unit)} ${shownUnit(unit)}`;
  if (step === "temperature") return `${formatDelta(1, "temperature", unit)} ${shownUnit(unit)}`;
  if (step === "pressure") return `${formatLocal(100, "pressure", unit)} ${shownUnit(unit)}`;
  if (step === "humidity") return "5 %";
  if (step === "target_speed") return `${formatLocal(0.5, "velocity", unit)} ${shownUnit(unit)}`;
  return "1×";
}

function recordField(record, name, label, dim, fallbackUnit) {
  return qtyField(
    name,
    label,
    record[`${name}_text`] || "",
    record[`${name}_unit`] || fallbackUnit,
    dim,
    record[`${name}_si`],
    record[`${name}_display`],
    name === "zero_range" ? "zero_range" : ""
  );
}

function groupDigits(value, digits) {
  const number = Number(value);
  if (!Number.isFinite(number)) return value == null || value === "" ? "—" : String(value);
  const text = digits == null ? String(Math.round(number * 10000) / 10000) : number.toFixed(digits);
  const neg = text.startsWith("-");
  const body = neg ? text.slice(1) : text;
  const parts = body.split(".");
  const grouped = parts[0].replace(/\B(?=(\d{3})+(?!\d))/g, "\u202f");
  const frac = parts.length > 1 ? `.${parts[1]}` : "";
  return `${neg ? "−" : ""}${grouped}${frac}`;
}

function holdClicks(hold) {
  if (!hold || hold.clicks == null || hold.clicks === "") return "";
  const count = Math.abs(Number(hold.clicks));
  if (!Number.isFinite(count)) return "";
  const text = tf("hold-clicks", { dir: hold.direction || "", count: String(count) });
  return esc(text).replace(esc(String(count)), `<b>${esc(String(count))}</b>`);
}

function exactAngular(hold) {
  if (!hold) return "";
  const angular = ((boot.settings && boot.settings.angular) || "mrad").toLowerCase();
  const key = { moa: "moa", iphy: "iphy", "cm@100m": "cm_per_100m", clicks: "mrad" }[angular] || "mrad";
  const number = Number(hold[key]);
  return Number.isFinite(number) ? Math.abs(number).toFixed(4) : "";
}

function figureParts(hold) {
  const primary = (hold && hold.primary) || "—";
  // The hyphen is escaped. An unescaped "-" between "+" and "−" is a range
  // that includes the digits, so the optional sign group ate the first digit.
  const match = String(primary).match(/^([+\-\u2212]?)(\d+(?:[.,]\d+)?)\s*(.*)$/);
  if (!match) return { sign: "", num: primary, unit: "" };
  return { sign: match[1] || "", num: match[2], unit: match[3] || "" };
}

function figureDigits(unit) {
  const text = String(unit || "").toLowerCase();
  if (!text || text === "clicks") return 0;
  if (text.includes("cm/100") || text.includes("cm@100")) return 1;
  return 2;
}

function figureShown(hold) {
  const parts = figureParts(hold);
  const number = Number(String(parts.num).replace(",", "."));
  if (!Number.isFinite(number)) return { num: parts.num, unit: parts.unit };
  return { num: Math.abs(number).toFixed(figureDigits(parts.unit)), unit: parts.unit };
}

function dirGlyph(direction) {
  return { UP: "▲", DOWN: "▼", LEFT: "◀", RIGHT: "▶" }[direction] || "";
}

function readoutHtml(hold) {
  const figure = figureShown(hold);
  const sign = figureParts(hold).sign;
  const shownSign = sign === "+" ? "" : sign;
  const glyph = dirGlyph(hold && hold.direction);
  const exact = exactAngular(hold);
  return `<div class="top"><span class="ho-dir">${glyph ? `${glyph} ` : ""}${esc((hold && hold.direction) || "—")}</span><span class="ho-exact">${esc(exact)}</span></div>
    <div class="ho-fig">${esc(shownSign)}${esc(figure.num)}${figure.unit ? `<span class="u">${esc(figure.unit)}</span>` : ""}</div>
    <div class="ho-clicks">${holdClicks(hold)}</div>`;
}

function sgBadge(sg) {
  const number = Number(sg);
  if (!Number.isFinite(number)) return "";
  if (number >= 1.5) return `<span class="ho-badge ok">${esc(t("sg-stable"))}</span>`;
  if (number >= 1.0) return `<span class="ho-badge warn">${esc(t("sg-marginal"))}</span>`;
  return `<span class="ho-badge err">${esc(t("sg-unstable"))}</span>`;
}

function kv(label, value, unit) {
  return `<dt>${label}</dt><dd>${esc(value)}</dd><span class="ho-unit">${esc(unit || "")}</span>`;
}

function trajectoryHtml(solve) {
  if (!solve) return kv(esc(t("trajectory")), "—", "");
  const sg = Number(solve.sg);
  return [
    kv(esc(t("velocity")), groupDigits(solve.velocity_display, 0), shownUnit(solve.velocity_unit)),
    kv(esc(t("mach")), Number.isFinite(Number(solve.mach)) ? Number(solve.mach).toFixed(2) : "—", ""),
    kv(esc(t("energy")), groupDigits(solve.energy_display, 0), shownUnit(solve.energy_unit)),
    kv(esc(t("tof")), Number.isFinite(Number(solve.tof_s)) ? Number(solve.tof_s).toFixed(3) : "—", "s"),
    kv(esc(t("drop")), groupDigits(solve.drop_display, 1), shownUnit(solve.drop_unit)),
    kv(`${esc(t("stability"))} ${sgBadge(sg)}`, Number.isFinite(sg) ? sg.toFixed(2) : t("no-sg"), ""),
    kv(esc(t("density-altitude")), groupDigits(solve.density_altitude_display, 0), shownUnit(solve.density_altitude_unit)),
  ].join("");
}

function clicksCell(hold) {
  if (!hold || hold.clicks == null || hold.clicks === "") return "—";
  const count = Math.abs(Number(hold.clicks));
  if (!Number.isFinite(count)) return "—";
  return `${hold.direction} · ${count}`;
}

function primaryNumber(hold) {
  return figureShown(hold).num || "—";
}

function sensHeading(hold) {
  const angular = (boot.settings && boot.settings.angular) || "mrad";
  const unit = t(angularKey(angular));
  const dir = hold && hold.direction && hold.direction !== "—" ? hold.direction : "";
  return `${dir} ${unit}`.trim();
}

function sensitivityHtml(solve) {
  if (!solve) return "";
  const length = units().length;
  const velocity = units().velocity;
  const items = solve.sensitivity || [];
  const rangeRows = items.filter((item) => item.kind === "range");
  const windRows = items.filter((item) => item.kind === "wind");
  const currentRange = formatLocal(Number(solve.range_m), "length", length);
  const windInput = document.getElementById("wind_speed");
  const parsedWind = windInput ? parseLocal(windInput.value, "velocity", windInput.dataset.unit) : null;
  const currentWind = parsedWind ? parsedWind.si : lastGood.windSi;
  const windText = Number.isFinite(currentWind) ? Number(formatLocal(currentWind, "velocity", velocity)).toFixed(1) : "—";
  const rangeBody = [];
  const pushRange = (label, hold, current) => {
    rangeBody.push(`<tr class="${current ? "current" : ""}"><td>${esc(label)}</td><td>${esc(primaryNumber(hold))}</td><td>${esc(clicksCell(hold))}</td></tr>`);
  };
  rangeRows.filter((item) => item.label === "minus").forEach((item) => pushRange(`${formatLocal(item.range_m, "length", length)} ${shownUnit(length)}`, item.elevation, false));
  pushRange(`${currentRange} ${shownUnit(length)}`, solve.elevation, true);
  rangeRows.filter((item) => item.label === "plus").forEach((item) => pushRange(`${formatLocal(item.range_m, "length", length)} ${shownUnit(length)}`, item.elevation, false));
  const windBody = [];
  const pushWind = (label, hold, current) => {
    windBody.push(`<tr class="${current ? "current" : ""}"><td>${esc(label)}</td><td>${esc(primaryNumber(hold))}</td><td>${esc(clicksCell(hold))}</td></tr>`);
  };
  windRows.filter((item) => item.label === "slower").forEach((item) => {
    const text = Number(formatLocal(item.speed_mps, "velocity", velocity)).toFixed(1);
    if (text === windText) return;
    pushWind(`${text} ${shownUnit(velocity)}`, item.windage, false);
  });
  pushWind(`${windText} ${shownUnit(velocity)}`, solve.windage, true);
  windRows.filter((item) => item.label === "faster").forEach((item) => {
    pushWind(`${Number(formatLocal(item.speed_mps, "velocity", velocity)).toFixed(1)} ${shownUnit(velocity)}`, item.windage, false);
  });
  const elevHead = sensHeading(solve.elevation);
  const windHead = sensHeading(solve.windage);
  return `<table class="ho-table"><caption>${esc(tf("sensitivity-range-caption", { step: stepAmount("range", length), name: t("status-range-step") }))}</caption>
      <thead><tr><th>${esc(t("range"))}</th><th>${esc(elevHead)}</th><th>${esc(t("clicks-heading"))}</th></tr></thead><tbody>${rangeBody.join("")}</tbody></table>
    <table class="ho-table"><caption>${esc(tf("sensitivity-wind-caption", { step: stepAmount("wind_speed", velocity), name: t("status-wind-step") }))}</caption>
      <thead><tr><th>${esc(t("wind-speed"))}</th><th>${esc(windHead)}</th><th>${esc(t("clicks-heading"))}</th></tr></thead><tbody>${windBody.join("")}</tbody></table>`;
}

function contextHtml() {
  const range = document.getElementById("range");
  const wind = document.getElementById("wind_speed");
  const dir = document.getElementById("wind_direction");
  const angle = document.getElementById("inclination");
  const hour = dir ? clockHour(dir.value) : "";
  const rangeText = range ? `${range.value} ${shownUnit(range.dataset.unit)}` : "";
  const windParsed = wind ? parseLocal(wind.value, "velocity", wind.dataset.unit) : null;
  const windText = windParsed
    ? `${wind.value} ${shownUnit(wind.dataset.unit)} @ ${dir ? clockPhrase(dir.value) : hour}`
    : (lastGood.wind ? `${lastGood.wind} @ ${dir ? clockPhrase(dir.value) : hour}` : "");
  const angleText = angle ? `${angle.value}${shownUnit(angle.dataset.unit) === "deg" ? "°" : " " + shownUnit(angle.dataset.unit)}` : "";
  const air = (boot.settings.current_air && boot.settings.current_air.provenance && boot.settings.current_air.provenance.source === "fetched")
    ? t("weather-fetched")
    : t("atmosphere-manual");
  const row = (label, value) => `<div><dt>${esc(label)}</dt><dd>${esc(value)}</dd></div>`;
  return `${row(t("range"), rangeText)}${row(t("wind-speed"), windText)}${row(t("inclination"), angleText)}${row(t("solve-tab-air"), air)}`;
}

function zeroWindSpin(solve) {
  if (!solve || !solve.windage) return false;
  if (!effects().spin_drift) return false;
  const wind = document.getElementById("wind_speed");
  const parsed = wind ? parseLocal(wind.value, "velocity", wind.dataset.unit) : null;
  const speed = parsed ? parsed.si : lastGood.windSi;
  if (speed == null || Math.abs(speed) > 0.05) return false;
  const mrad = Math.abs(Number(solve.windage.mrad));
  return Number.isFinite(mrad) && mrad > 0.0005;
}

function notesHtml(solve) {
  const items = [];
  const warnings = [];
  if (solve) {
    (solve.warnings || []).forEach((item) => { if (item) warnings.push(item); });
    if (solve.station_pressure_note) warnings.push(solve.station_pressure_note);
  }
  if (!warnings.length) items.push(`<li class="ok">${esc(t("notes-none"))}</li>`);
  warnings.forEach((item) => items.push(`<li class="warn">${esc(item)}</li>`));
  if (zeroWindSpin(solve)) items.push(`<li class="info">${esc(t("spin-zero-wind"))}</li>`);
  return items.join("");
}

function paintSolve() {
  const elev = document.getElementById("hold-elev");
  const wind = document.getElementById("hold-wind");
  if (!elev || !wind) return;
  elev.innerHTML = readoutHtml(lastSolve && lastSolve.elevation);
  wind.innerHTML = readoutHtml(lastSolve && lastSolve.windage);
  elev.classList.toggle("ho-stale", stale);
  wind.classList.toggle("ho-stale", stale);
  const traj = document.getElementById("traj-card");
  const sens = document.getElementById("sens-card");
  if (traj) traj.classList.toggle("ho-stale", stale);
  if (sens) sens.classList.toggle("ho-stale", stale);
  const ctx = document.getElementById("hold-ctx");
  if (ctx) ctx.innerHTML = contextHtml();
  const meta = document.getElementById("traj-meta");
  const range = document.getElementById("range");
  if (meta && range) meta.textContent = tf("at-range", { range: `${range.value} ${shownUnit(range.dataset.unit)}` });
  const trajBody = document.getElementById("traj-body");
  if (trajBody) trajBody.innerHTML = trajectoryHtml(lastSolve);
  const sensBody = document.getElementById("sens-body");
  if (sensBody) sensBody.innerHTML = sensitivityHtml(lastSolve);
  const notes = document.getElementById("solve-notes");
  if (notes) notes.innerHTML = notesHtml(lastSolve);
  const error = document.getElementById("solve-error");
  if (error) {
    error.hidden = !stale || !solveError;
    error.innerHTML = stale && solveError ? `<p class="ho-reason" role="status"><span>●</span><span>${esc(solveError)}</span></p>` : "";
  }
  const differs = document.getElementById("differs");
  if (differs && lastSolve) differs.classList.toggle("hidden", !lastSolve.current_differs);
  const truing = document.getElementById("truing-line");
  if (truing && lastSolve) {
    const text = lastSolve.truing || t("truing-none");
    truing.innerHTML = lastSolve.truing ? esc(String(text)) : `<span class="ho-badge warn">${esc(String(text))}</span>`;
  }
  document.querySelectorAll("[data-angular]").forEach((button) => {
    const angular = (boot.settings && boot.settings.angular) || "mrad";
    const on = button.dataset.angular === angular;
    button.setAttribute("aria-checked", on ? "true" : "false");
    button.tabIndex = on ? 0 : -1;
  });
}

function kindKey(input) {
  const dim = (input && (input.dataset.kind || input.dataset.dim)) || "number";
  return { length: "kind-length", velocity: "kind-speed", temperature: "kind-temperature", pressure: "kind-pressure", angle: "kind-angle", percent: "kind-percent", count: "kind-count" }[dim] || "kind-number";
}

function invalidQuantity(input) {
  const sample = (String(input.value).match(/[+-]?\d+(?:[.,]\d+)?/) || [""])[0];
  if (!sample) return t("invalid-number");
  let suffix = "";
  if (input.dataset.kind === "percent") suffix = " %";
  else if (input.dataset.unit) suffix = ` ${shownUnit(input.dataset.unit)}`;
  else if (input.dataset.dim === "velocity") suffix = " m/s";
  else if (input.dataset.dim === "temperature") suffix = " °C";
  return tf("invalid-quantity", { kind: t(kindKey(input)), sample, suffix });
}

function staleSentence(input) {
  const labelNode = input.id && document.querySelector(`label[for="${input.id}"]`);
  const last = lastGoodText(input);
  return tf("stale-hold", {
    field: labelNode ? labelNode.textContent : t("range"),
    text: input.value,
    kind: t(kindKey(input)),
    last,
  });
}

function lastGoodText(input) {
  if (input.dataset.dim === "velocity" && lastGood.wind) return lastGood.wind;
  if (input.dataset.text) return `${input.dataset.text}`.replace(/\bmps\b/, "m/s");
  return "—";
}

let lastGood = { wind: "", windSi: null };

function rememberGood() {
  const wind = document.getElementById("wind_speed");
  if (!wind) return;
  const parsed = parseLocal(wind.value, "velocity", wind.dataset.unit);
  if (!parsed) return;
  lastGood.windSi = parsed.si;
  lastGood.wind = `${formatLocal(parsed.si, "velocity", wind.dataset.unit)} ${shownUnit(wind.dataset.unit)}`;
}

function fieldIsInvalid(input) {
  if (!input) return false;
  const text = String(input.value || "").trim();
  const field = input.closest(".ho-field");
  if (field && field.dataset.stepper === "humidity") {
    if (!text) return false;
    const raw = text.replace(",", ".");
    const match = raw.match(/^([+-]?(?:\d+(?:\.\d+)?|\.\d+))\s*(%)?$/);
    if (!match) return true;
    const value = Number(match[1]);
    if (!Number.isFinite(value)) return true;
    const percent = (match[2] || Math.abs(value) > 1) ? value : value * 100;
    return percent < 0 || percent > 100;
  }
  if (field && (field.dataset.stepper === "wind_direction" || field.dataset.stepper === "compare-direction")) {
    if (!text) return true;
    return !clockOk(text);
  }
  if (!input.dataset.dim) return false;
  if (!text) return input.id !== "target_speed";
  return !parseLocal(input.value, input.dataset.dim, input.dataset.unit);
}

function markInvalid(input, bad) {
  const field = input.closest(".ho-field");
  if (field) field.classList.toggle("invalid", bad);
  input.setAttribute("aria-invalid", bad ? "true" : "false");
  const hint = field && field.querySelector(".ho-hint");
  if (!hint) return;
  if (bad) hint.textContent = invalidQuantity(input);
  else if (hint.dataset.base) hint.textContent = hint.dataset.base;
}

function syncWindClock(hour) {
  document.querySelectorAll('.ho-clock button[data-hour]').forEach((button) => {
    button.setAttribute("aria-checked", String(Number(button.dataset.hour) === hour));
  });
}

function leadNote() {
  const preset = (boot.running || []).find((item) => item.id === moving.preset);
  return preset ? preset.note : t("lead-clock");
}

function leadHold() {
  const speed = document.getElementById("target_speed");
  const typed = speed ? String(speed.value || "").trim() : String(moving.speedText || "").trim();
  if (!typed && !(lastSolve && lastSolve.lead)) return "";
  const lead = lastSolve && lastSolve.lead;
  if (!lead || !lead.flight_lead) {
    return `<div class="lead-result"><p class="direction">${esc(t("lead"))}</p><p>${esc(t("lead-waiting"))}</p></div>`;
  }
  const mrad = Number(lead.mrad);
  const moa = Number(lead.moa);
  const cm = Number(lead.cm);
  const tof = Number(lead.tof_s);
  const along = Number.isFinite(mrad) && Math.abs(mrad) < 0.0005 ? `<p class="meta">${esc(t("lead-along"))}</p>` : "";
  const impactRange = lead.impact_range_display
    ? `${esc(lead.impact_range_display)} ${esc(lead.length_unit || "")}`
    : "—";
  return `<div class="lead-result">
    <p class="direction">${esc(t("lead"))} ${esc(lead.flight_lead.direction)}</p>
    <p class="figure">${esc(formatLead(mrad))} mrad</p>
    <p class="clicks">${esc(formatLead(moa))} MOA · ${esc(formatLead(cm))} cm ${esc(t("lead-at-target"))}</p>
    <p class="clicks">${esc(t("tof"))} ${Number.isFinite(tof) ? tof.toFixed(3) : "—"} s · ${esc(t("lead-impact"))} ${impactRange}</p>
    <p class="clicks">${esc(t("lead-combined"))} ${esc(lead.combined.direction)} ${esc(lead.combined.primary)}</p>
    ${along}
    <p class="meta">${esc(lead.target && lead.target.id ? t("lead-outline-drawn") : t("lead-outline"))} ${esc(lead.reaction_note || "")}</p>
    <div id="lead-face"></div>
  </div>`;
}

function formatLead(value) {
  if (!Number.isFinite(value)) return "—";
  const abs = Math.abs(value);
  const text = abs >= 100 ? String(Math.round(abs * 10) / 10) : String(Math.round(abs * 100) / 100);
  return value < -1e-9 ? `−${text}` : text;
}

function cardHtml() {
  const s = boot.settings;
  const pages = (boot.card_pages || []).map((item) =>
    `<option value="${esc(item.id)}" ${item.id === cardPageId ? "selected" : ""}>${esc(item.name)}</option>`).join("");
  const rail = `<h2>${esc(t("nav-card"))}</h2>
    ${qtyField("from", t("card-from"), "100 m", units().length, "length", 100, null, "from")}
    ${qtyField("to", t("card-to"), "600 m", units().length, "length", 600, null, "to")}
    ${qtyField("step", t("card-step"), "50 m", units().length, "length", 50, null, "step")}
    <div class="ho-field"><div class="head"><label for="brackets">${esc(t("card-winds"))}</label></div><div class="ho-entry"><input id="brackets" value="${esc(cardBrackets)}" autocomplete="off" /></div></div>
    ${hoSelect("card-page", t("card-size"), pages)}
    <div class="ho-field"><div class="head"><label for="conditions">${esc(t("card-conditions"))}</label></div><div class="ho-entry"><input id="conditions" value="${esc(cardConditions)}" placeholder="-10 C, 30 C" autocomplete="off" /></div></div>
    <label class="ho-check"><input type="checkbox" id="card-pin" ${s.card_pinned ? "checked" : ""} /> ${esc(t("card-pin"))}</label>
    <div class="btnrow">
      <button type="button" class="ho-btn primary" id="card-build">${esc(t("apply"))}</button>
      <button type="button" class="ho-btn" id="card-pdf" title="${esc(t("card-pdf-tip"))}">${esc(t("card-pdf"))}</button>
    </div>
    <div class="btnrow">
      <button type="button" class="ho-btn ghost" id="card-print" title="${esc(t("card-print-note"))}">${esc(t("card-print"))}</button>
      <button type="button" class="ho-btn ghost" id="card-csv">${esc(t("card-csv"))}</button>
      <button type="button" class="ho-btn ghost" id="card-json">${esc(t("card-json"))}</button>
    </div>
    <p class="ho-hint">${esc(t("page-pin-note"))} ${esc(t("card-paper-note"))}</p>
    <p class="ho-hint">${esc(t("card-print-note"))}</p>
    <p class="ho-hint" id="card-pdf-status"></p>`;
  const main = `<div id="card-body">${card && card.rows ? cardTable(card) : emptyCta("card-empty")}</div>`;
  return frame({ rail, main, railLabel: t("nav-card") });
}

function clickNote(hold) {
  if (!hold || hold.clicks == null || hold.clicks === "") return "";
  const count = Math.abs(Number(hold.clicks));
  if (!Number.isFinite(count)) return "";
  return `<small>${esc(String(count))}</small>`;
}

function cardHoldCell(hold, headerDir) {
  if (!hold) return "—";
  const shown = figureShown(hold);
  const dir = hold.direction && hold.direction !== "—" && hold.direction !== headerDir ? `${hold.direction} ` : "";
  return `${esc(dir)}${esc(shown.num)}${clickNote(hold)}`;
}

function cardTable(data) {
  const current = Number(boot.settings && boot.settings.last_range_si);
  const elevDir = data.rows[0] && data.rows[0].elevation && data.rows[0].elevation.direction;
  const elevUnit = figureShown(data.rows[0] && data.rows[0].elevation).unit || shownUnit(data.angular);
  const heads = (data.brackets || []).map((label, index) => {
    const sample = data.rows[0] && data.rows[0].winds && data.rows[0].winds[index] && data.rows[0].winds[index].windage;
    const dir = sample && sample.direction && sample.direction !== "—" ? sample.direction : "";
    const note = index === 0 ? `<small>${esc(t("card-wind-note"))}</small>` : "";
    return `<th class="group-start">${esc(`${dir} ${prettyPhrase(label)}`.trim())}${note}</th>`;
  }).join("");
  const conditionHeads = (data.conditions || []).map((label) => `<th class="group-start">${esc(prettyPhrase(label))}</th>`).join("");
  const rows = data.rows.map((row) => {
    const on = Number.isFinite(current) && Number.isFinite(Number(row.range_m)) && Math.abs(Number(row.range_m) - current) < 0.51;
    return `<tr class="${on ? "current" : ""}">
      <td>${esc(fixedDisplay(row.range_display, 0))} ${esc(shownUnit(data.length_unit))}${on ? ` <span class="ho-badge accent">${esc(t("card-current"))}</span>` : ""}</td>
      <td>${cardHoldCell(row.elevation, elevDir)}</td>
      <td>${esc(fixedDisplay(row.drop_display, 2))}</td>
      <td>${esc(fixedDisplay(row.velocity_display, 2))}</td>
      <td>${esc(fixedDisplay(row.energy_display, 2))}</td>
      <td>${Number(row.tof_s).toFixed(3)}</td>
      <td>${Number(row.mach).toFixed(2)}</td>
      ${(row.winds || []).map((wind, index) => {
        const sample = data.rows[0] && data.rows[0].winds && data.rows[0].winds[index] && data.rows[0].winds[index].windage;
        const dir = sample && sample.direction;
        return `<td class="group-start">${cardHoldCell(wind.windage, dir)}</td>`;
      }).join("")}
      ${(row.conditions || []).map((item) => `<td class="group-start">${cardHoldCell(item.elevation, item.elevation && item.elevation.direction)}</td>`).join("")}
    </tr>`;
  }).join("");
  const barMm = Number(data.scale_bar_mm);
  const barLabel = esc(data.scale_bar_label || "");
  const bar = barMm
    ? `<div class="scale-bar scale-bar-screen" style="width:${barMm}mm" aria-hidden="true"><span>${barLabel}</span></div>
       <p class="ho-hint">${barLabel}</p>`
    : "";
  return `<section class="ho-card flush">
      <header><h2>${esc(data.profile_label || t("nav-card"))}</h2><span class="meta">${esc(t("card-preview-only"))}</span></header>
      <div class="ho-table-wrap"><table class="ho-table zebra"><thead><tr>
        <th>${esc(t("range"))}</th><th>${esc(`${elevDir && elevDir !== "—" ? elevDir : ""} ${elevUnit}`.trim())}</th><th>${esc(t("drop"))}<small>${esc(t("drop-bore"))}</small></th>
        <th>${esc(t("velocity"))}</th><th>${esc(t("energy"))}</th><th>${esc(t("tof"))}</th><th>${esc(t("mach"))}</th>
        ${heads}${conditionHeads}
      </tr></thead><tbody>${rows}</tbody></table></div>
    </section>
    <div class="grid2">
      <section class="ho-card">
        <header><h2>${esc(t("card-conditions-title"))}</h2></header>
        <dl class="ho-kv">
          <dt>${esc(t("zero"))}</dt><dd>${esc(prettyPhrase(data.zero_summary || t("missing-not-set")))}</dd><span class="ho-unit"></span>
          <dt>${esc(t("truing"))}</dt><dd>${esc(data.truing || t("truing-none"))}</dd><span class="ho-unit"></span>
          <dt>${esc(t("documents-date"))}</dt><dd>${esc(localDate(data.date))}</dd><span class="ho-unit"></span>
        </dl>
        <p class="ho-hint">${esc(t("from-zero"))}. ${esc(t("crosswind"))}.</p>
      </section>
      <section class="ho-card">
        <header><h2>${esc(t("card-scale-title"))}</h2></header>
        ${bar}
        <p class="ho-hint">${esc(t("card-scale-hint"))}</p>
      </section>
    </div>
    <p class="ho-hint">${esc(data.footer || t("range-card-footer"))}</p>
    <p class="ho-hint">${esc(data.disclaimer || boot.disclaimer)}</p>`;
}

function chartHtml() {
  const profiles = boot.equipment.profiles || [];
  const boxes = profiles.map((item) => `<label><input type="checkbox" class="overlay" value="${esc(item.id)}" ${item.id === boot.settings.active_profile_id ? "checked" : ""} /> ${esc(item.label)}</label>`).join("");
  const kindTitle = {
    drop: "chart-drop-tip",
    wind: "chart-wind-tip",
    velocity: "chart-velocity-tip",
    energy: "chart-energy-tip",
    tof: "chart-tof-tip",
    mach: "chart-mach-tip",
  };
  const kinds = ["drop", "wind", "velocity", "energy", "tof", "mach"].map((kind) => {
    const tip = esc(t(kindTitle[kind]));
    const label = kind === "drop" ? "chart-drop" : kind === "wind" ? "chart-wind" : kind;
    return `<label title="${tip}"><input type="checkbox" class="chart-kind" value="${kind}" title="${tip}" ${chartKinds[kind] ? "checked" : ""} /> ${esc(t(label))}</label>`;
  }).join("");
  const rail = `<h2>${esc(t("nav-charts"))}</h2>
    <div class="checks">${boxes}</div>
    <div class="checks">${kinds}</div>
    <div class="btnrow">
      <button type="button" class="ho-btn primary" id="chart-build">${esc(t("apply"))}</button>
      <button type="button" class="ho-btn ghost" id="chart-svg">${esc(t("download-svg"))}</button>
      <button type="button" class="ho-btn ghost" id="chart-png">${esc(t("download-png"))}</button>
      <button type="button" class="ho-btn ghost" id="chart-csv">${esc(t("card-csv"))}</button>
    </div>
    <p class="ho-hint">${esc(t("charts-note"))} ${esc(t("same-conditions"))} ${esc(t("chart-limit"))}</p>`;
  const groups = (boot.equipment.groups || []).map((item) => `<option value="${esc(item.id)}" ${item.id === impactGroupId ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const impact = `<div id="chart-pane-impact" class="pane${chartPane === "impact" ? "" : " is-hidden"}">
      <p class="ho-hint">${esc(t("impact-intro"))}</p>
      <div class="ho-fields">
        ${hoSelect("impact-target", t("impact-target"), `${targetOptions(impactTarget)}<option value="custom" ${impactTarget === "custom" ? "selected" : ""}>${esc(t("impact-custom"))}</option>`, t("impact-target-tip"))}
        ${hoSelect("impact-facing", t("facing"), `<option value="left" ${impactFacing === "left" ? "selected" : ""}>${esc(t("facing-left"))}</option><option value="right" ${impactFacing === "right" ? "selected" : ""}>${esc(t("facing-right"))}</option>`, t("facing-tip"))}
        ${hoSelect("impact-ringing", t("ringing"), `<option value="primary" ${impactRinging === "primary" ? "selected" : ""}>${esc(t("ringing-primary"))}</option><option value="alt" ${impactRinging === "alt" ? "selected" : ""}>${esc(t("ringing-alt"))}</option>`, t("ringing-tip"))}
      </div>
      <label class="ho-check" title="${esc(t("silhouette-tip"))}"><input id="impact-silhouette" type="checkbox" ${impactSilhouette ? "checked" : ""} /> ${esc(t("silhouette"))}</label>
      <div class="ho-fields">
        <span title="${esc(t("impact-range-tip"))}">${qtyField("impact-range", t("range"), boot.settings.last_range || "300 m", units().length, "length", null, null, "impact-range")}</span>
        <span title="${esc(t("impact-mv-tip"))}">${qtyField("mv_sd", t("impact-mv-sd"), "0 mps", units().velocity, "velocity")}</span>
        <span title="${esc(t("impact-error-tip"))}">${qtyField("range_error", t("impact-range-error"), "1 m", units().length, "length", 1)}</span>
        <div class="ho-field" title="${esc(t("impact-wind-tip"))}"><div class="head"><label for="impact-brackets">${esc(t("card-winds"))}</label></div><div class="ho-entry"><input id="impact-brackets" value="2 mps, 4 mps" title="${esc(t("impact-wind-tip"))}" autocomplete="off" /></div></div>
        <span title="${esc(t("impact-zone-tip"))}">${qtyField("impact-zone", t("pbr-zone"), "20 cm", units().small, "length")}</span>
        ${hoSelect("impact-group", t("group-saved"), `<option value="">${esc(t("group-blank"))}</option>${groups}`, t("group-impact"))}
      </div>
      <button type="button" class="ho-btn" id="impact-draw" title="${esc(t("impact-draw-tip"))}">${esc(t("impact-draw"))}</button>
      <div id="impact-body"></div>
    </div>`;
  const main = `<div class="ho-tabs" role="tablist" aria-label="${esc(t("nav-charts"))}">
      <button type="button" role="tab" data-chart-pane="trajectory" aria-selected="${chartPane === "trajectory"}">${esc(t("chart-tab-trajectory"))}</button>
      <button type="button" role="tab" data-chart-pane="impact" aria-selected="${chartPane === "impact"}">${esc(t("impact-title"))}</button>
    </div>
    <div id="chart-pane-traj" class="pane${chartPane === "trajectory" ? "" : " is-hidden"}">
      <div class="grid2">
        <section class="ho-card">
          <p class="legend" id="chart-legend"></p>
          <div id="chart-body"></div>
        </section>
        <section class="ho-card" id="chart-readout" aria-label="${esc(t("chart-at"))}">
          <header><h2>${esc(t("chart-at"))}</h2></header>
          <div id="chart-table"><p class="ho-hint">${esc(t("chart-cursor"))}</p></div>
        </section>
      </div>
    </div>
    ${impact}`;
  return frame({ rail, main, railLabel: t("nav-charts") });
}

function equipmentHtml() {
  const tabs = ["rifle", "scope", "bullet", "load", "profile", "chrono", "recoil", "log", "groups", "archive"].map((kind) =>
    `<button type="button" role="tab" data-kind="${kind}" aria-selected="${equipKind === kind}">${esc(t("equip-" + kind))}</button>`).join("");
  const tablist = `<div class="ho-tabs" role="tablist" aria-label="${esc(t("nav-equipment"))}">${tabs}</div>`;
  if (equipKind === "archive") {
    return frame({
      rail: `<h2>${esc(t("equip-archive"))}</h2><p class="ho-hint">${esc(t("archive-intro"))}</p>`,
      main: `${tablist}<section class="ho-card">${archiveHtml()}</section>`,
      body: "with-list",
      railLabel: t("nav-equipment"),
    });
  }
  const items = equipList().map((item) =>
    `<button type="button" data-pick="${esc(item.id)}" aria-selected="${item.id === selectedId}"><span>${esc(item.label)}</span><span class="sub">${esc(t("equip-" + equipKind))}</span></button>`).join("");
  const library = equipKind === "recoil" ? "" : `<div class="ho-list">${items || `<p class="ho-hint">${esc(t("equip-empty"))}</p>`}</div>`;
  const newer = equipKind === "recoil" || equipKind === "chrono" || equipKind === "groups" ? "" : `<button type="button" class="ho-btn" id="equip-new">${esc(t("new"))}</button>`;
  return frame({
    rail: `<h2>${esc(t("nav-equipment"))}</h2>${newer}${library}`,
    main: `${tablist}<section class="ho-card" id="equip-form">${equipForm()}</section>`,
    body: "with-list",
    railLabel: t("nav-equipment"),
  });
}

function equipList() {
  if (equipKind === "log") return boot.equipment.load_logs || [];
  if (equipKind === "groups") return boot.equipment.groups || [];
  return boot.equipment[equipKind + "s"] || boot.equipment[equipKind] || [];
}

function currentRecord() {
  const list = equipKind === "log" ? (boot.equipment.load_logs || []) : equipKind === "groups" ? (boot.equipment.groups || []) : (boot.equipment[equipKind + "s"] || []);
  return list.find((item) => item.id === selectedId) || null;
}

function equipForm() {
  if (equipKind === "groups") return groupsEquipHtml();
  if (equipKind === "chrono") return chronoHtml();
  if (equipKind === "recoil") return recoilHtml();
  if (equipKind === "log") return logForm();
  let record = currentRecord();
  if (!record && !equipDraft) {
    return `<p class="ho-hint">${esc(t("equip-pick"))}</p>`;
  }
  record = record || {};
  const id = record.id ? `<input type="hidden" id="record-id" value="${esc(record.id)}" />` : `<input type="hidden" id="record-id" value="" />`;
  let fields = "";
  if (equipKind === "rifle") {
    fields = textInput("label", t("label"), record.label) + textInput("make_model", t("make-model"), record.make_model)
      + recordField(record, "barrel_length", t("barrel"), "length", units().small)
      + textInput("twist", t("twist"), record.twist_text)
      + hoSelect("twist_hand", t("twist-hand"), `<option value="right" ${record.twist_hand !== "left" ? "selected" : ""}>RH</option><option value="left" ${record.twist_hand === "left" ? "selected" : ""}>LH</option>`)
      + recordField(record, "sight_height", t("sight-height"), "length", units().small)
      + recordField(record, "weight", t("weight"), "mass", units().mass)
      + textInput("serial", t("serial"), record.serial) + `<p class="hint">${esc(t("serial-note"))}</p>`;
  } else if (equipKind === "scope") {
    fields = textInput("label", t("label"), record.label) + textInput("make_model", t("make-model"), record.make_model)
      + hoSelect("focal_plane", t("focal-plane"), `<option value="ffp" ${record.focal_plane !== "sfp" ? "selected" : ""}>${esc(t("ffp"))}</option><option value="sfp" ${record.focal_plane === "sfp" ? "selected" : ""}>${esc(t("sfp"))}</option>`)
      + `<p class="hint">${esc(t("sfp-note"))}</p>`
      + numberInput("mag_min", t("magnification") + " min", record.mag_min)
      + numberInput("mag_max", t("magnification") + " max", record.mag_max)
      + numberInput("calibrated_mag", t("calibrated-mag"), record.calibrated_mag)
      + recordField(record, "click", t("click"), "angle", units().angle)
      + numberInput("tracking_factor", t("tracking"), record.tracking_factor ?? 1)
      + recordField(record, "travel_up", t("travel-up"), "angle", units().angle)
      + recordField(record, "travel_down", t("travel-down"), "angle", units().angle)
      + recordField(record, "travel_left", t("travel-left"), "angle", units().angle)
      + recordField(record, "travel_right", t("travel-right"), "angle", units().angle);
  } else if (equipKind === "bullet") {
    const bands = record.bands || [];
    const second = bands[1] || {};
    fields = textInput("label", t("label"), record.label) + textInput("make_model", t("make-model"), record.make_model)
      + recordField(record, "diameter", t("diameter"), "length", units().small)
      + recordField(record, "weight", t("weight"), "mass", units().mass)
      + recordField(record, "length", t("length"), "length", units().small)
      + hoSelect("drag_model", t("drag-model"), ["G1", "G7", "G2", "G5", "G6", "G8"].map((model) => `<option ${record.drag_model === model ? "selected" : ""}>${model}</option>`).join(""))
      + numberInput("bc", t("bc"), bands[0] ? bands[0].bc : "")
      + hoSelect("bc_reference", t("bc-reference"), `<option ${record.bc_reference !== "Army" ? "selected" : ""}>ICAO</option><option ${record.bc_reference === "Army" ? "selected" : ""}>Army</option>`)
      + numberInput("bc2", t("bc-second"), second.bc ?? "")
      + qtyField("bc2_min", t("bc-min"), second.min_text || "", "mps", "velocity")
      + `<p class="hint">${esc(t("bc-hint"))} ${esc(t("band-hint"))}</p>`
      + textInput("source_note", t("source-note"), record.source_note);
  } else if (equipKind === "load") {
    const bullets = boot.equipment.bullets.map((item) => `<option value="${esc(item.id)}" ${item.id === record.bullet_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
    fields = textInput("label", t("label"), record.label)
      + hoSelect("bullet_id", t("bullet-link"), bullets)
      + recordField(record, "mv", t("mv"), "velocity", units().velocity)
      + recordField(record, "mv_temperature", t("mv-temperature"), "temperature", units().temperature)
      + recordField(record, "sensitivity", t("powder-sensitivity"), "velocity", units().velocity)
      + `<p class="hint">${esc(t("powder-hint"))}</p>`
      + textInput("source_note", t("source-note"), record.source_note);
  } else {
    const select = (id, label, list, current) => hoSelect(id, label, list.map((item) => `<option value="${esc(item.id)}" ${item.id === current ? "selected" : ""}>${esc(item.label)}</option>`).join(""));
    const air = record.zero_air || {};
    fields = textInput("label", t("label"), record.label)
      + select("rifle_id", t("rifle-link"), boot.equipment.rifles, record.rifle_id)
      + select("scope_id", t("scope-link"), boot.equipment.scopes, record.scope_id)
      + select("load_id", t("load-link"), boot.equipment.loads, record.load_id)
      + recordField(record, "zero_range", t("zero-range"), "length", units().length)
      + recordField(record, "zero_offset", t("zero-offset"), "length", units().small)
      + `<p class="hint">${esc(t("offset-hint"))}</p>`
      + `<p class="meta">${esc(t("zero-air"))}</p>`
      + airFields(air, "zero_")
      + recordField(record, "zero_powder", t("zero-powder"), "temperature", units().temperature)
      + `<p class="hint">${esc(t("zero-saved-on-profile"))}</p>`;
  }
  const notice = equipNotice;
  equipNotice = "";
  return `${id}<div class="ho-fields">${fields}</div>
    <div class="btnrow"><button type="button" class="ho-btn primary" id="equip-save">${esc(t("save"))}</button>
    ${record.id ? `<button type="button" class="ho-btn" id="equip-delete">${esc(t("archive"))}</button>` : ""}</div>
    <p id="equip-msg" class="meta">${esc(notice)}</p>`;
}

function textInput(id, label, value) {
  return `<div class="ho-field"><div class="head"><label for="${id}">${esc(label)}</label></div><div class="ho-entry"><input id="${id}" value="${esc(value || "")}" autocomplete="off" /></div></div>`;
}

function numberInput(id, label, value) {
  if (id === "mag_min" || id === "mag_max" || id === "calibrated_mag") {
    const input = `<input id="${id}" value="${esc(value ?? "")}" inputmode="decimal" autocomplete="off" spellcheck="false" />`;
    return hoQty(id, label, "", "count", id, input);
  }
  return `<div class="ho-field"><div class="head"><label for="${id}">${esc(label)}</label></div><div class="ho-entry"><input id="${id}" class="num" value="${esc(value ?? "")}" inputmode="decimal" autocomplete="off" /></div></div>`;
}

function hoSelect(id, label, options, title) {
  const tip = title ? ` title="${esc(title)}"` : "";
  return `<div class="ho-field"${tip}><div class="head"><label for="${id}">${esc(label)}</label></div><div class="ho-entry"><select id="${id}"${tip}>${options}</select></div></div>`;
}

function settingsHtml() {
  const s = boot.settings;
  const rings = boot.targets || { moose_cm: [], roe_cm: [] };
  const lat = s.weather_latitude == null ? "" : String(s.weather_latitude);
  const lon = s.weather_longitude == null ? "" : String(s.weather_longitude);
  const rangeParsed = parseLocal(s.range_step || "10 m", "length", "m");
  const windParsed = parseLocal(s.wind_step || "1 mps", "velocity", "mps");
  const main = `<p class="ho-hint">${esc(t("settings-page-intro"))}</p>
    <div class="cols">
      <section class="ho-card stack">
        <header><h2>${esc(t("settings-appearance"))}</h2></header>
        <div class="ho-seg" role="radiogroup" aria-label="${esc(t("settings-appearance"))}">
          ${["follow", "light", "dark"].map((mode) => {
            const on = s.appearance === mode;
            return `<button type="button" role="radio" data-appearance="${mode}" aria-checked="${on}" tabindex="${on ? "0" : "-1"}">${esc(t("settings-" + mode))}</button>`;
          }).join("")}
        </div>
        <p class="ho-hint">${esc(t("follow-note"))}</p>
        <label class="ho-check"><input type="checkbox" id="large-text" ${s.large_text ? "checked" : ""} /> ${esc(t("settings-large"))}</label>
        <label class="ho-check"><input type="checkbox" id="reduced-motion" ${s.reduced_motion ? "checked" : ""} /> ${esc(t("settings-motion"))}</label>
        <p class="ho-hint">${esc(t("palette"))}: ${esc(theme.source || "")} · ${esc(t("text-scale"))} ${esc(theme.text_scale || 1)}</p>
      </section>
      <section class="ho-card stack">
        <header><h2>${esc(t("settings-steps"))}</h2></header>
        ${qtyField("range-step", t("settings-range-step"), "", "m", "length", rangeParsed ? rangeParsed.si : 10)}
        ${qtyField("wind-step", t("settings-wind-step"), "", "mps", "velocity", windParsed ? windParsed.si : 1)}
        <p class="ho-hint">${esc(t("settings-steps-hint"))}</p>
        <header><h2>${esc(t("settings-units"))}</h2></header>
        <div id="settings-units" class="ho-fields">${settingsUnitFields()}</div>
      </section>
      <section class="ho-card stack">
        <header><h2>${esc(t("settings-weather"))}</h2></header>
        <p class="ho-hint">${esc(t("settings-weather-intro"))}</p>
        <div class="ho-fields">
          <div class="ho-field wide"><div class="head"><label for="weather-place">${esc(t("weather-place"))}</label></div><div class="ho-entry"><input id="weather-place" value="${esc(s.weather_place || "")}" title="${esc(t("weather-place-tip"))}" autocomplete="off" /></div></div>
          <div class="ho-field"><div class="head"><label for="weather-lat">${esc(t("weather-latitude"))}</label></div><div class="ho-entry"><input id="weather-lat" class="num" value="${esc(lat)}" inputmode="decimal" title="${esc(t("weather-latitude-tip"))}" autocomplete="off" /></div></div>
          <div class="ho-field"><div class="head"><label for="weather-lon">${esc(t("weather-longitude"))}</label></div><div class="ho-entry"><input id="weather-lon" class="num" value="${esc(lon)}" inputmode="decimal" title="${esc(t("weather-longitude-tip"))}" autocomplete="off" /></div></div>
          <div class="ho-field"><div class="head"><label for="weather-stale">${esc(t("weather-stale-minutes"))}</label></div><div class="ho-entry"><input id="weather-stale" class="num" value="${esc(s.weather_stale_minutes || 60)}" inputmode="numeric" title="${esc(t("weather-stale-tip"))}" autocomplete="off" /></div></div>
        </div>
        <button type="button" class="ho-btn" id="weather-save" title="${esc(t("weather-save-tip"))}">${esc(t("weather-save"))}</button>
        <p id="weather-save-msg" class="meta"></p>
        <p class="ho-hint">${esc(t("weather-attribution"))}</p>
        <p class="ho-hint">${esc(t("weather-noncommercial"))}</p>
        <header><h2>${esc(t("settings-library"))}</h2></header>
        <p class="ho-hint">${esc(t("settings-library-intro"))}</p>
        <label class="ho-check"><input type="checkbox" id="include-location" title="${esc(t("include-location-tip"))}" /> ${esc(t("include-location"))}</label>
        <div class="btnrow">
          <button type="button" class="ho-btn" id="export">${esc(t("settings-export"))}</button>
          <button type="button" class="ho-btn" id="export-csv" title="${esc(t("settings-library-intro"))}">${esc(t("settings-export-csv"))}</button>
          <button type="button" class="ho-btn ghost" id="show-json">${esc(t("open-backup"))}</button>
        </div>
        <p id="backup-path" class="meta"></p>
        <pre id="backup-json" class="hidden"></pre>
        <p class="ho-hint">${esc(t("include-note"))}</p>
        <div class="ho-field"><div class="head"><label for="restore-file">${esc(t("settings-restore"))}</label></div><input id="restore-file" type="file" accept="application/json,.json,text/csv,.csv" title="${esc(t("settings-library-intro"))}" /></div>
        <pre id="restore-preview"></pre>
        <div class="btnrow">
          <button type="button" class="ho-btn hidden" id="restore-commit">${esc(t("settings-replace"))}</button>
          <button type="button" class="ho-btn hidden" id="restore-merge">${esc(t("import-merge"))}</button>
        </div>
        <p class="ho-hint">${esc(t("restore-warning"))}</p>
        <header><h2>${esc(t("targets-title"))}</h2></header>
        <p>${esc(t("rings-moose"))}: ${esc((rings.moose_cm || []).join(" / "))} cm</p>
        <p>${esc(t("rings-roe"))}: ${esc((rings.roe_cm || []).join(" / "))} cm</p>
        <p class="ho-hint">${esc(t("targets-note"))} ${esc(t("older-rings"))}</p>
        <header><h2>${esc(t("nav-about"))}</h2></header>
        <p class="ho-hint">${esc(t("settings-about"))}</p>
        <button type="button" class="ho-btn" id="open-about-settings">${esc(t("nav-about"))}</button>
      </section>
    </div>`;
  return frame({ body: "no-rail", main, railLabel: t("nav-settings") });
}

function onboardHtml() {
  const choice = (id, title, detail) => {
    const on = id === "metric";
    return `<button type="button" class="ho-choice" role="radio" data-onboard="${id}" aria-checked="${on}" tabindex="${on ? "0" : "-1"}"><b>${esc(t(title))}</b><span>${esc(t(detail))}</span></button>`;
  };
  return `<a class="ho-skip" href="#onboard-title">${esc(t("skip"))}</a>
    <div class="ho-shell" inert>
      <header class="ho-top"><span class="brand">${esc(t("app-name"))}</span></header>
      <div class="ho-body no-rail"><main class="ho-main" id="main"></main></div>
      <footer class="ho-status"><span class="ho-legal">${esc(boot.disclaimer || "")}</span></footer>
    </div>
    <div class="ho-scrim" role="dialog" aria-modal="true" aria-labelledby="onboard-title">
      <div class="ho-dialog">
        <header><h2 id="onboard-title">${esc(t("onboard-title"))}</h2></header>
        <div class="body">
          <div role="radiogroup" aria-label="${esc(t("onboard-title"))}">
            ${choice("metric", "onboard-metric", "onboard-metric-detail")}
            ${choice("us", "onboard-us", "onboard-us-detail")}
            ${choice("custom", "onboard-custom", "onboard-custom-detail")}
          </div>
          <div class="ho-seg wrap" role="radiogroup" aria-label="${esc(t("onboard-example"))}">
            <button type="button" role="radio" data-example="yes" aria-checked="true" tabindex="0">${esc(t("onboard-example-yes"))}</button>
            <button type="button" role="radio" data-example="no" aria-checked="false" tabindex="-1">${esc(t("onboard-example-no"))}</button>
          </div>
          <input type="checkbox" id="example" class="hidden" checked />
          <div id="onboard-custom" class="hidden">${unitsPopover(false)}</div>
          <p class="ho-hint">${esc(t("example-note"))}</p>
          <p class="ho-hint">${esc(boot.disclaimer)}</p>
          <p class="ho-hint" id="onboard-custom-note"></p>
        </div>
        <footer><button type="button" class="ho-btn primary" id="onboard-accept">${esc(t("onboard-accept"))}</button></footer>
      </div>
    </div>`;
}

function bindOnboard() {
  let preset = "metric";
  document.querySelectorAll("[data-example]").forEach((button) => {
    button.addEventListener("click", () => {
      document.getElementById("example").checked = button.dataset.example === "yes";
      document.querySelectorAll("[data-example]").forEach((item) => item.setAttribute("aria-checked", item === button ? "true" : "false"));
    });
  });
  document.querySelectorAll("[data-onboard]").forEach((button) => {
    button.addEventListener("click", () => {
      preset = button.dataset.onboard;
      document.querySelectorAll("[data-onboard]").forEach((item) => item.setAttribute("aria-checked", item === button ? "true" : "false"));
      document.getElementById("onboard-custom").classList.toggle("hidden", preset !== "custom");
      document.getElementById("onboard-accept").disabled = false;
    });
  });
  document.getElementById("onboard-accept").addEventListener("click", async () => {
    if (!preset) return;
    const body = { preset, example: document.getElementById("example").checked };
    if (preset === "custom") {
      const custom = {};
      document.querySelectorAll("#onboard-custom [data-unit-key]").forEach((select) => {
        custom[select.dataset.unitKey] = select.value;
      });
      body.units = custom;
      body.angular = custom.angular;
    }
    boot = await call("complete_onboarding", { body });
    render();
  });
  bindRoving("[data-onboard], [data-example]");
  const first = document.querySelector("[data-onboard][aria-checked=true]");
  if (first) first.focus();
}

function bindRoving(selector) {
  const groups = new Map();
  document.querySelectorAll(selector).forEach((button) => {
    const group = button.closest("[role=radiogroup]") || button.parentElement;
    if (!groups.has(group)) groups.set(group, []);
    groups.get(group).push(button);
  });
  groups.forEach((buttons) => {
    const current = Math.max(0, buttons.findIndex((button) => button.getAttribute("aria-checked") === "true"));
    buttons.forEach((button, index) => {
      button.setAttribute("role", "radio");
      button.tabIndex = index === current ? 0 : -1;
      button.addEventListener("keydown", (event) => {
        if (!["ArrowRight", "ArrowDown", "ArrowLeft", "ArrowUp"].includes(event.key)) return;
        event.preventDefault();
        const delta = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : -1;
        const next = buttons[(index + delta + buttons.length) % buttons.length];
        buttons.forEach((item) => { item.tabIndex = item === next ? 0 : -1; });
        next.focus();
        next.click();
      });
    });
  });
}

function bindShell() {
  document.querySelectorAll("[data-page]").forEach((button) => {
    button.addEventListener("click", () => { page = button.dataset.page; paletteOpen = false; render(); });
  });
  const about = document.getElementById("open-about");
  if (about) about.addEventListener("click", () => { page = "about"; render(); });
  document.querySelectorAll("[data-preset]").forEach((button) => {
    button.addEventListener("click", () => setPreset(button.dataset.preset));
  });
  const unitsButton = document.getElementById("units-open");
  if (unitsButton) {
    unitsButton.addEventListener("click", () => {
      unitsOpen = !unitsOpen;
      render();
    });
  }
  document.querySelectorAll(".ho-screens").forEach((button) => {
    button.addEventListener("click", () => {
      paletteOpen = true;
      render();
      fillPalette();
    });
  });
  const apply = document.getElementById("units-apply");
  if (apply) {
    apply.addEventListener("click", async () => {
      const custom = {};
      document.querySelectorAll("#units-pop [data-unit-key]").forEach((select) => { custom[select.dataset.unitKey] = select.value; });
      boot = await call("set_preferences", { body: { preset: "custom", units: custom, angular: custom.angular, appearance: boot.settings.appearance, large_text: boot.settings.large_text, reduced_motion: boot.settings.reduced_motion, card_pinned: boot.settings.card_pinned } });
      unitsOpen = false;
      render();
    });
  }
  bindRoving("[data-preset], [data-angular], [data-appearance], [data-example], [data-onboard]");
}

function bindPage() {
  document.querySelectorAll(".chip").forEach((chip) => chip.addEventListener("click", () => cycleChip(chip)));
  document.querySelectorAll("input[data-dim]").forEach((input) => {
    input.addEventListener("input", () => {
      input.dataset.dirty = "1";
      showPreview(input);
      if (page === "solve") {
        if (input.id === "temperature" || input.id === "pressure" || input.id === "altitude") noteAirEdit();
        scheduleSolve(120);
      }
      if (page === "compare") {
        rememberCompareNumbers();
        scheduleCompare(150);
      }
      if (page === "groups") scheduleGroup(160);
    });
    input.addEventListener("change", () => {
      if (page === "solve") scheduleSolve(0);
      if (page === "compare") scheduleCompare(0);
      if (page === "groups") scheduleGroup(0);
    });
  });
  if (page === "solve") bindSolve();
  if (page === "card") bindCard();
  if (page === "charts") bindChart();
  if (page === "reticle") bindReticle();
  if (page === "targets") bindTargets();
  if (page === "groups") bindGroups();
  if (page === "pbr") bindPbr();
  if (page === "truing") bindTruing();
  if (page === "equipment") bindEquipment();
  if (page === "compare") bindCompare();
  if (page === "documents") bindDocuments();
  if (page === "sync") bindSync();
  if (page === "settings") bindSettings();
  wireSteppers();
}

function wireSteppers(root) {
  if (!window.attachSteppers) return;
  attachSteppers(root || document.getElementById("app"), {
    parse: parseLocal,
    format: formatLocal,
    settings: stepSettings,
    changed(input) {
        input.dataset.dirty = "1";
        if (input.id === "target_speed") {
          moving.open = true;
          moving.speedText = input.value;
          moving.speedUnit = input.dataset.unit || moving.speedUnit;
        }
        if (input.id === "reticle-mag") reticleState.mag = Number(input.value) || reticleState.mag;
        if (input.id === "reticle-cal") reticleState.cal = Number(input.value) || reticleState.cal;
        if (input.id === "reticle-range") reticleState.rangeText = input.value;
        if (input.id === "compare-direction") compareForm.direction = input.value;
        if (input.id === "pbr-zero") pbrZeroText = input.value;
        if (input.closest(".ho-field")) markInvalid(input, fieldIsInvalid(input));
        showPreview(input);
        if (page === "solve") {
          if (input.id === "temperature" || input.id === "pressure" || input.id === "altitude" || input.id === "humidity") noteAirEdit();
          scheduleSolve(0);
        }
        if (page === "compare") {
          rememberCompareNumbers();
          scheduleCompare(0);
        }
        if (page === "groups") scheduleGroup(0);
        if (page === "reticle") scheduleReticle();
        if (input.id === "range-step" || input.id === "wind-step") saveSteps();
    },
  });
}

function bindSolve() {
  ["profile", "wind_direction", "pressure_kind", "humidity"].forEach((id) => {
    const node = document.getElementById(id);
    if (!node) return;
    node.addEventListener("input", () => {
      if (id === "profile") {
        const chosen = (boot.equipment.profiles || []).find((item) => item.id === node.value);
        const facts = document.getElementById("profile-facts");
        if (facts) facts.innerHTML = profileFacts(chosen);
      }
      if (id === "pressure_kind" || id === "humidity") noteAirEdit();
      scheduleSolve(80);
    });
  });
  bindWeather();
  document.querySelectorAll("[data-effect]").forEach((box) => box.addEventListener("change", () => {
    document.getElementById("extra").classList.toggle("hidden", !needsExtra());
    scheduleSolve(0);
  }));
  document.getElementById("extra").classList.toggle("hidden", !needsExtra());
  const preset = document.getElementById("lead-preset");
  if (preset) preset.addEventListener("change", () => {
    moving.preset = preset.value;
    moving.open = true;
    const item = (boot.running || []).find((row) => row.id === preset.value);
    if (item && item.speed_known) {
      moving.speedText = `${item.speed_mps} mps`;
      moving.speedUnit = "mps";
      const speed = document.getElementById("target_speed");
      if (speed) {
        speed.value = formatLocal(Number(item.speed_mps), "velocity", "mps");
        speed.dataset.unit = "mps";
        speed.dataset.si = String(item.speed_mps);
        speed.dataset.text = `${speed.value} mps`;
        speed.dataset.dirty = "1";
        moving.speedText = speed.value;
        const chip = document.querySelector('[data-for="target_speed"]');
        if (chip) chip.textContent = shownUnit("mps");
      }
    }
    if (item && !item.speed_known) {
      moving.speedText = "";
      const speed = document.getElementById("target_speed");
      if (speed) speed.value = "";
    }
    const note = document.getElementById("lead-note");
    if (note) note.textContent = leadNote();
    scheduleSolve(0);
  });
  ["target_speed", "target_direction", "reaction_s"].forEach((id) => {
    const node = document.getElementById(id);
    if (!node) return;
    node.addEventListener("input", () => {
      moving.open = true;
      if (id === "target_speed") {
        moving.speedText = node.value;
        moving.speedUnit = node.dataset.unit;
        node.dataset.dirty = "1";
      } else if (id === "target_direction") moving.direction = node.value;
      else moving.reaction = node.value;
      scheduleSolve(80);
    });
  });
  const leadPick = document.getElementById("lead-target");
  if (leadPick) leadPick.addEventListener("change", () => {
    leadTarget = leadPick.value;
    moving.open = true;
    scheduleSolve(0);
  });
  const leadFace = document.getElementById("lead-facing");
  if (leadFace) leadFace.addEventListener("change", () => {
    leadFacing = leadFace.value;
    moving.open = true;
    scheduleSolve(0);
  });
  const copy = document.getElementById("copy-hold");
  if (copy) copy.addEventListener("click", () => copyHold());
  document.querySelectorAll("[data-angular]").forEach((button) => {
    button.addEventListener("click", () => setAngular(button.dataset.angular));
  });
  if (!window.__hoResize) {
    window.__hoResize = () => { if (page === "solve") selectSolveTab(solveTab); };
    window.addEventListener("resize", window.__hoResize);
  }
  bindSolveTabs();
  paintLeadFace();
  paintSolve();
}

function bindSolveTabs() {
  const list = document.querySelector(".solve-tabs [role=tablist]");
  if (!list) return;
  const tabs = [...list.querySelectorAll("[role=tab]")];
  tabs.forEach((button) => {
    button.addEventListener("click", () => selectSolveTab(button.dataset.tab, button));
    button.addEventListener("keydown", (event) => {
      const index = tabs.indexOf(button);
      let next = -1;
      if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
      else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = tabs.length - 1;
      if (next < 0) return;
      event.preventDefault();
      event.stopPropagation();
      selectSolveTab(tabs[next].dataset.tab, tabs[next]);
    });
  });
}

function selectSolveTab(id, focus) {
  solveTab = id;
  document.querySelectorAll(".solve-tabs [role=tab]").forEach((button) => {
    const on = button.dataset.tab === id;
    button.setAttribute("aria-selected", on ? "true" : "false");
    button.tabIndex = on ? 0 : -1;
  });
  const wide = window.matchMedia("(min-width: 1400px)").matches;
  document.querySelectorAll(".solve-tabs [role=tabpanel]").forEach((panel) => {
    const hide = !wide && panel.dataset.tab !== id;
    panel.classList.toggle("is-hidden", hide);
    panel.setAttribute("aria-hidden", hide ? "true" : "false");
  });
  if (focus) focus.focus();
}

function needsExtra() {
  const flags = effects();
  return flags.coriolis || flags.cant;
}

function bindCard() {
  document.getElementById("card-build").addEventListener("click", buildCard);
  document.getElementById("card-pdf").addEventListener("click", openCardPdf);
  document.getElementById("card-print").addEventListener("click", () => window.print());
  document.getElementById("card-csv").addEventListener("click", () => {
    if (card && card.csv) downloadText("holdover-card.csv", card.csv, "text/csv");
  });
  document.getElementById("card-json").addEventListener("click", () => {
    if (card) downloadText("holdover-card.json", JSON.stringify(card, null, 2), "application/json");
  });
  document.getElementById("card-pin").addEventListener("change", async (event) => {
    boot = await call("set_preferences", { body: preferencesBody({ card_pinned: event.target.checked }) });
  });
  const pageSelect = document.getElementById("card-page");
  if (pageSelect) pageSelect.addEventListener("change", async () => {
    cardPageId = pageSelect.value;
    cardPageTouched = true;
    boot = await call("set_preferences", { body: preferencesBody({ paper: cardPageId }) });
  });
  const conditions = document.getElementById("conditions");
  if (conditions) conditions.addEventListener("input", () => { cardConditions = conditions.value; });
  const brackets = document.getElementById("brackets");
  if (brackets) brackets.addEventListener("input", () => { cardBrackets = brackets.value; });
  if (!card && boot.settings.active_profile_id) buildCard();
}

function cardFormBody() {
  const body = solveBody();
  body.from = qtyPayload("from");
  body.to = qtyPayload("to");
  body.step = qtyPayload("step");
  body.brackets = document.getElementById("brackets").value;
  body.pinned_metric = document.getElementById("card-pin").checked;
  body.page = document.getElementById("card-page").value;
  body.conditions = document.getElementById("conditions").value;
  body.edition_date = editionDate();
  cardPageId = body.page;
  cardPageTouched = true;
  cardConditions = body.conditions;
  cardBrackets = body.brackets;
  return body;
}

async function buildCard() {
  const host = document.getElementById("card-body");
  try {
    const body = cardFormBody();
    card = await call("range_card", { body });
    applyCardPage(card);
    host.innerHTML = cardTable(card);
  } catch (error) {
    host.innerHTML = `<p class="ho-hint">${esc(t("card-error"))}</p><details><summary>${esc(t("card-error-detail"))}</summary><p class="error">${esc(error)}</p></details>`;
  }
}

async function openCardPdf() {
  const status = document.getElementById("card-pdf-status");
  try {
    const body = cardFormBody();
    const result = await call("card_pdf", { body });
    if (status) {
      const lead = result.opened ? t("card-pdf-opened") : t("card-pdf-saved");
      status.textContent = `${lead} ${result.path || ""}`.trim();
    }
    if (!result.opened && result.pdf_base64) downloadBase64("holdover-range-card.pdf", result.pdf_base64);
  } catch (error) {
    if (status) status.textContent = String(error);
  }
}

function downloadBase64(name, encoded) {
  const binary = atob(encoded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  const blob = new Blob([bytes], { type: "application/pdf" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
}

function bindChart() {
  document.querySelectorAll("[data-chart-pane]").forEach((button) => button.addEventListener("click", () => {
    chartPane = button.dataset.chartPane;
    document.querySelectorAll("[data-chart-pane]").forEach((item) => item.setAttribute("aria-selected", item === button ? "true" : "false"));
    const traj = document.getElementById("chart-pane-traj");
    const impact = document.getElementById("chart-pane-impact");
    if (traj) traj.classList.toggle("is-hidden", chartPane !== "trajectory");
    if (impact) impact.classList.toggle("is-hidden", chartPane !== "impact");
    if (chartPane === "trajectory" && chartData) paintCharts(true);
  }));
  document.getElementById("chart-build").addEventListener("click", buildChart);
  document.querySelectorAll(".overlay").forEach((box) => box.addEventListener("change", buildChart));
  document.querySelectorAll(".chart-kind").forEach((box) => box.addEventListener("change", () => {
    chartKinds[box.value] = box.checked;
    if (chartData) paintCharts(true);
  }));
  [
    ["impact-range", "impact-range-tip"],
    ["mv_sd", "impact-mv-tip"],
    ["range_error", "impact-error-tip"],
    ["impact-zone", "impact-zone-tip"],
  ].forEach(([id, key]) => {
    const input = document.getElementById(id);
    const chip = document.querySelector(`[data-for="${id}"]`);
    const tip = t(key);
    if (input) input.title = tip;
    if (chip) {
      chip.title = tip;
      chip.setAttribute("aria-label", tip);
    }
  });
  document.getElementById("chart-svg").addEventListener("click", () => downloadSvg("holdover-charts.svg"));
  document.getElementById("chart-png").addEventListener("click", () => downloadPng("holdover-charts.png"));
  document.getElementById("chart-csv").addEventListener("click", () => {
    if (chartData) downloadText("holdover-chart.csv", chartCsv(), "text/csv");
  });
  const redrawImpact = () => {
    const ids = [...document.querySelectorAll(".overlay:checked")].map((box) => box.value).slice(0, 6);
    buildImpact(ids);
  };
  const drawImpact = document.getElementById("impact-draw");
  if (drawImpact) drawImpact.addEventListener("click", redrawImpact);
  if (groupImpactPending) {
    groupImpactPending = false;
    redrawImpact();
  }
  const onImpactControl = (id, node) => {
    if (id === "impact-target") impactTarget = node.value;
    if (id === "impact-facing") impactFacing = node.value;
    if (id === "impact-ringing") impactRinging = node.value;
    if (id === "impact-group") impactGroupId = node.value;
    redrawImpact();
  };
  ["impact-target", "impact-facing", "impact-ringing", "impact-group"].forEach((id) => {
    const node = document.getElementById(id);
    if (!node) return;
    node.addEventListener("change", () => onImpactControl(id, node));
  });
  const brackets = document.getElementById("impact-brackets");
  if (brackets) brackets.addEventListener("input", () => redrawImpact());
  const silhouette = document.getElementById("impact-silhouette");
  if (silhouette) silhouette.addEventListener("change", () => {
    impactSilhouette = silhouette.checked;
    redrawImpact();
  });
  ["impact-range", "mv_sd", "range_error", "impact-zone"].forEach((id) => {
    const node = document.getElementById(id);
    if (node) node.addEventListener("input", () => redrawImpact());
  });
  const host = document.getElementById("chart-body");
  if (window.ResizeObserver) {
    if (chartObserver) chartObserver.disconnect();
    chartObserver = new ResizeObserver(() => {
      if (page === "charts" && chartData) paintCharts();
    });
    chartObserver.observe(host);
  }
  buildChart();
}

async function buildChart() {
  const checked = [...document.querySelectorAll(".overlay:checked")].map((box) => box.value);
  const ids = checked.slice(0, 6);
  const body = solveBody();
  body.profile_ids = ids;
  body.from = { text: "0 m", unit: "m" };
  body.to = { text: "1000 m", unit: "m" };
  body.step = { text: "50 m", unit: "m" };
  const host = document.getElementById("chart-body");
  if (!ids.length) {
    chartData = null;
    host.innerHTML = (boot.equipment.profiles || []).length ? `<p class="ho-hint">${esc(t("chart-empty"))}</p>` : emptyCta("chart-empty");
    return;
  }
  try {
    chartData = await call("chart", { body });
    chartCursor = 0;
    chartPaintWidth = 0;
    host.dataset.limit = checked.length > 6 ? "1" : "";
    paintCharts();
    await buildImpact(ids);
  } catch (error) {
    host.innerHTML = `<p class="error">${esc(error)}</p>`;
  }
}

function paintCharts(force) {
  const host = document.getElementById("chart-body");
  if (!host || !chartData || chartPainting) return;
  const width = Math.max(280, host.clientWidth || 640);
  if (!force && width === chartPaintWidth && host.querySelector("svg")) return;
  chartPainting = true;
  chartPaintWidth = width;
  const note = host.dataset.limit === "1" ? `<p class="hint">${esc(t("chart-six"))}</p>` : "";
  const kinds = ["drop", "wind", "velocity", "energy", "tof", "mach"].filter((kind) => chartKinds[kind]);
  host.innerHTML = note + (kinds.length ? kinds.map((kind) => drawScaledChart(kind)).join("") : `<p>${esc(t("chart-kinds-empty"))}</p>`);
  bindChartPointers();
  positionCursor();
  writeReadouts();
  const legend = document.getElementById("chart-legend");
  if (legend) legend.innerHTML = legendItems();
  chartPainting = false;
}

function legendItems() {
  const series = (chartData.series || []).map((item, index) =>
    `<span><i class="swatch" style="border-color:var(--chart-${index % 6})"></i>${esc(item.label)}</span>`).join("");
  const shade = chartData && chartData.transonic != null && chartData.transonic !== ""
    ? `<span><i class="swatch transonic-swatch" title="${esc(t("chart-transonic"))}"></i>${esc(t("chart-transonic"))}</span>`
    : "";
  return series + shade;
}

function xLabelStride(count, plotW) {
  if (count <= 2) return 1;
  return Math.max(1, Math.ceil(52 / (plotW / (count - 1))));
}

function drawScaledChart(kind) {
  const spec = chartData[kind];
  const xAxis = chartData.x;
  const yAxis = spec.axis;
  const width = chartPaintWidth;
  const height = width < 520 ? 248 : 286;
  const padL = width < 420 ? 50 : 62;
  const padR = 12;
  const padT = 16;
  const padB = 26;
  const plotW = Math.max(40, width - padL - padR);
  const plotH = Math.max(40, height - padT - padB);
  const xSpan = (xAxis.max - xAxis.min) || 1;
  const ySpan = (yAxis.max - yAxis.min) || 1;
  const xOf = (value) => padL + (value - xAxis.min) / xSpan * plotW;
  const yOf = (value) => padT + (yAxis.max - value) / ySpan * plotH;
  chartGeom[kind] = {
    padL, padT, plotW, plotH, width, height,
    xMin: xAxis.min, xMax: xAxis.max, yMin: yAxis.min, yMax: yAxis.max,
  };
  const yKey = kind === "drop" ? "path" : kind === "wind" ? "windage" : kind;
  const stride = xLabelStride(xAxis.ticks.length, plotW);
  const last = xAxis.ticks.length - 1;
  let marks = `<rect class="frame" x="${padL}" y="${padT}" width="${plotW.toFixed(1)}" height="${plotH.toFixed(1)}" fill="none" />`;
  const transonic = chartData.transonic;
  if (transonic !== null && transonic !== undefined && Number.isFinite(Number(transonic))) {
    const x0 = Math.min(width - padR, Math.max(padL, xOf(Number(transonic))));
    marks += `<rect class="transonic" x="${x0.toFixed(1)}" y="${padT}" width="${Math.max(0, width - padR - x0).toFixed(1)}" height="${plotH.toFixed(1)}" />`;
  }
  yAxis.ticks.forEach((tick, index) => {
    const y = yOf(tick);
    marks += `<line class="grid" x1="${padL}" x2="${(width - padR).toFixed(1)}" y1="${y.toFixed(1)}" y2="${y.toFixed(1)}" />`;
    marks += `<text class="tick" fill="var(--text-secondary)" x="${padL - 8}" y="${(y + 4).toFixed(1)}" text-anchor="end">${esc(yAxis.labels[index])}</text>`;
  });
  xAxis.ticks.forEach((tick, index) => {
    const x = xOf(tick);
    marks += `<line class="grid" y1="${padT}" y2="${(height - padB).toFixed(1)}" x1="${x.toFixed(1)}" x2="${x.toFixed(1)}" />`;
    const crowded = index === last - 1 && last % stride !== 0 && stride > 1;
    if ((index % stride === 0 || index === last) && !crowded) {
      const anchor = index === last ? "end" : index === 0 ? "start" : "middle";
      const labelX = index === 0 ? padL : index === last ? width - padR : x;
      marks += `<text class="tick" fill="var(--text-secondary)" x="${labelX.toFixed(1)}" y="${height - 6}" text-anchor="${anchor}">${esc(xAxis.labels[index])}</text>`;
    }
  });
  const zeroY = yOf(0);
  if (zeroY >= padT - 1 && zeroY <= height - padB + 1) {
    marks += `<line class="zero" x1="${padL}" x2="${(width - padR).toFixed(1)}" y1="${zeroY.toFixed(1)}" y2="${zeroY.toFixed(1)}" />`;
    if (spec.zero === "los" && width >= 420) {
      marks += `<text class="zero-label" fill="var(--text-secondary)" x="${(width - padR - 4).toFixed(1)}" y="${Math.max(padT + 12, zeroY - 5).toFixed(1)}" text-anchor="end">${esc(t("chart-los"))}</text>`;
    }
  }
  const paths = (chartData.series || []).map((item, index) => {
    const d = (item.points || []).map((point, i) =>
      `${i ? "L" : "M"}${xOf(point.range).toFixed(1)},${yOf(point[yKey]).toFixed(1)}`).join(" ");
    return `<path d="${d}" fill="none" stroke="var(--chart-${index % 6})" stroke-width="2" stroke-dasharray="${DASH[index % DASH.length]}" /><circle class="dot hidden" data-dot="${index}" r="3.5" fill="var(--chart-${index % 6})" cx="0" cy="0" />`;
  }).join("");
  const title = kind === "drop" ? t("chart-drop") : kind === "wind" ? t("chart-wind") : t(kind);
  const yTitle = kind === "drop"
    ? `${spec.unit}, ${t("chart-above-los")}`
    : kind === "wind"
      ? `${spec.unit}, ${t("chart-wind-sign")}`
      : `${spec.unit}`;
  const clicks = spec.clicks_axis ? `<p class="hint">${esc(t("chart-clicks-axis"))}</p>` : "";
  return `<section class="chart-block">
    <h3>${esc(title)}</h3>
    <p class="legend">${legendItems()}</p>
    <p class="axis-title">${esc(yTitle)}</p>
    <svg class="chart" data-chart="${kind}" data-ykey="${yKey}" viewBox="0 0 ${width} ${height}" role="img" tabindex="0" aria-label="${esc(title)}, ${esc(yTitle)}">
      ${marks}${paths}<line class="cursor hidden" x1="0" x2="0" y1="${padT}" y2="${padT + plotH}" />
    </svg>
    <p class="axis-title">${esc(t("range"))} (${esc(xAxis.unit)})</p>
    ${clicks}
    <p class="readout" data-readout="${kind}"></p>
  </section>`;
}

function bindChartPointers() {
  document.querySelectorAll("svg[data-chart]").forEach((svg) => {
    svg.addEventListener("pointermove", (event) => {
      const index = indexFromClient(svg, event.clientX);
      if (index !== null) setCursor(index);
    });
    svg.addEventListener("pointerdown", (event) => {
      svg.focus();
      const index = indexFromClient(svg, event.clientX);
      if (index !== null) setCursor(index);
    });
    svg.addEventListener("keydown", (event) => {
      const count = chartPointCount();
      if (!count) return;
      if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
        event.preventDefault();
        const delta = event.key === "ArrowRight" ? 1 : -1;
        const next = chartCursor === null ? 0 : chartCursor + delta;
        setCursor(Math.max(0, Math.min(count - 1, next)));
      } else if (event.key === "Home") {
        event.preventDefault();
        setCursor(0);
      } else if (event.key === "End") {
        event.preventDefault();
        setCursor(count - 1);
      }
    });
  });
}

function chartPointCount() {
  const series = chartData && chartData.series && chartData.series[0];
  return series && series.points ? series.points.length : 0;
}

function indexFromClient(svg, clientX) {
  const geom = chartGeom[svg.dataset.chart];
  const series = chartData && chartData.series && chartData.series[0];
  if (!geom || !series || !series.points.length) return null;
  const rect = svg.getBoundingClientRect();
  if (!rect.width) return null;
  const x = (clientX - rect.left) * (geom.width / rect.width);
  const range = geom.xMin + (x - geom.padL) / geom.plotW * (geom.xMax - geom.xMin);
  let best = 0;
  let bestDist = Infinity;
  series.points.forEach((point, index) => {
    const dist = Math.abs(point.range - range);
    if (dist < bestDist) {
      bestDist = dist;
      best = index;
    }
  });
  return best;
}

function setCursor(index) {
  chartCursor = index;
  positionCursor();
  writeReadouts();
}

function positionCursor() {
  if (!chartData || chartCursor === null) return;
  const sample = chartData.series[0] && chartData.series[0].points[chartCursor];
  if (!sample) return;
  document.querySelectorAll("svg[data-chart]").forEach((svg) => {
    const geom = chartGeom[svg.dataset.chart];
    const line = svg.querySelector(".cursor");
    if (!geom || !line) return;
    const x = geom.padL + (sample.range - geom.xMin) / ((geom.xMax - geom.xMin) || 1) * geom.plotW;
    line.setAttribute("x1", x.toFixed(1));
    line.setAttribute("x2", x.toFixed(1));
    line.setAttribute("y1", String(geom.padT));
    line.setAttribute("y2", String(geom.padT + geom.plotH));
    line.classList.remove("hidden");
    const key = svg.dataset.ykey || "path";
    svg.querySelectorAll(".dot").forEach((dot) => {
      const series = chartData.series[Number(dot.dataset.dot)];
      const point = series && series.points[chartCursor];
      if (!point) return;
      const y = geom.padT + (geom.yMax - point[key]) / ((geom.yMax - geom.yMin) || 1) * geom.plotH;
      dot.setAttribute("cx", x.toFixed(1));
      dot.setAttribute("cy", y.toFixed(1));
      dot.classList.remove("hidden");
    });
  });
}

function writeReadouts() {
  const table = document.getElementById("chart-table");
  if (table && chartData && chartData.series && chartData.series.length) {
    if (chartCursor === null) {
      table.innerHTML = `<p class="ho-hint">${esc(t("chart-cursor"))}</p>`;
    } else {
      const sample = chartData.series[0].points[chartCursor];
      const head = chartData.series.map((item) => `<th>${esc(item.label)}</th>`).join("");
      const row = (label, pick) => `<tr><th>${esc(label)}</th>${chartData.series.map((item) => {
        const point = item.points[chartCursor];
        return `<td>${point ? esc(pick(point)) : ""}</td>`;
      }).join("")}</tr>`;
      table.innerHTML = sample ? `<p class="ho-cap">${esc(t("chart-at"))} ${esc(fixedDisplay(sample.range, 0))} ${esc(shownUnit(chartData.x.unit))}</p>
        <div class="ho-table-wrap"><table class="ho-table"><thead><tr><th></th>${head}</tr></thead><tbody>
          ${row(t("drop"), (point) => directed(point.path, "UP", "DOWN", chartData.drop.unit))}
          ${row(t("elevation"), (point) => directed(point.elevation, "UP", "DOWN", chartData.drop.hold_unit))}
          ${row(t("windage"), (point) => directed(point.windage, "RIGHT", "LEFT", chartData.wind.unit))}
          ${row(t("velocity"), (point) => `${fixedDisplay(point.velocity, 0)} ${shownUnit(chartData.velocity.unit)}`)}
          ${row(t("energy"), (point) => `${fixedDisplay(point.energy, 0)} ${shownUnit(chartData.energy.unit)}`)}
          ${row(t("tof"), (point) => `${fixedDisplay(point.tof, 3)} s`)}
          ${row(t("mach"), (point) => fixedDisplay(point.mach, 2))}
        </tbody></table></div>` : "";
    }
  }
  document.querySelectorAll("[data-readout]").forEach((node) => {
    const kind = node.dataset.readout;
    if (!chartData) return;
    if (chartCursor === null) {
      node.textContent = t("chart-cursor");
      return;
    }
    const spec = chartData[kind] || {};
    const yKey = kind === "drop" ? "path" : kind === "wind" ? "windage" : kind;
    const range = chartData.series[0].points[chartCursor].range;
    const parts = chartData.series.map((item) => {
      const point = item.points[chartCursor];
      if (!point) return "";
      if (kind === "drop") {
        return `${item.label}: ${directed(point.path, "UP", "DOWN", chartData.drop.unit)} · ${directed(point.elevation, "UP", "DOWN", chartData.drop.hold_unit)}`;
      }
      if (kind === "wind") {
        return `${item.label}: ${directed(point.windage, "RIGHT", "LEFT", chartData.wind.unit)} · ${directed(point.wind_linear, "RIGHT", "LEFT", chartData.wind.linear_unit)}`;
      }
      const unit = kind === "tof" ? "s" : shownUnit(spec.unit);
      return `${item.label}: ${fixedDisplay(point[yKey], kind === "mach" ? 2 : kind === "tof" ? 3 : 0)} ${unit}`.trim();
    }).filter(Boolean);
    node.textContent = `${fixedDisplay(range, 0)} ${shownUnit(chartData.x.unit)} — ${parts.join("  ·  ")}`;
  });
}

function directed(value, positive, negative, unit) {
  const number = Number(value);
  if (!Number.isFinite(number)) return "—";
  const digits = Math.abs(number) >= 100 ? 1 : 2;
  const text = Math.abs(number).toFixed(digits);
  const word = Math.abs(number) < 1e-9 ? "" : (number > 0 ? positive : negative);
  return `${word} ${text} ${shownUnit(unit)}`.trim();
}

function signed(value) {
  if (!Number.isFinite(Number(value))) return "—";
  const number = Number(value);
  const abs = Math.abs(number);
  const text = abs >= 100 ? String(Math.round(abs * 10) / 10) : String(Math.round(abs * 1000) / 1000);
  return number < -1e-9 ? `−${text}` : text;
}

function bindEquipment() {
  document.querySelectorAll("[data-kind]").forEach((button) => button.addEventListener("click", () => {
    equipKind = button.dataset.kind;
    selectedId = "";
    equipDraft = false;
    render();
  }));
  document.querySelectorAll("[data-pick]").forEach((button) => button.addEventListener("click", () => {
    selectedId = button.dataset.pick;
    equipDraft = false;
    render();
  }));
  const newer = document.getElementById("equip-new");
  if (newer) newer.addEventListener("click", () => { selectedId = ""; equipDraft = true; render(); });
  const save = document.getElementById("equip-save");
  if (save) save.addEventListener("click", saveEquipment);
  const remove = document.getElementById("equip-delete");
  if (remove) remove.addEventListener("click", deleteEquipment);
  if (equipKind === "chrono") bindChrono();
  if (equipKind === "recoil") bindRecoil();
  if (equipKind === "log") bindLog();
  if (equipKind === "archive") bindArchive();
  const openGroupButton = document.getElementById("equip-open-group");
  if (openGroupButton) openGroupButton.addEventListener("click", () => openGroup(selectedId));
  const newGroupButton = document.getElementById("equip-new-group");
  if (newGroupButton) newGroupButton.addEventListener("click", () => {
    groupView = freshGroup();
    page = "groups";
    render();
  });
  document.querySelectorAll("[data-open-group]").forEach((button) => button.addEventListener("click", () => openGroup(button.dataset.openGroup)));
}

async function saveEquipment() {
  const msg = document.getElementById("equip-msg");
  try {
    const body = equipPayload();
    const command = equipKind === "log" ? "save_load_log" : "save_" + equipKind;
    const saved = await call(command, { body });
    selectedId = saved.id;
    equipNotice = saved.warning ? `${t("saved")} ${saved.warning}` : t("saved");
    boot = await call("bootstrap");
    render();
  } catch (error) {
    if (msg) msg.textContent = String(error);
  }
}

async function deleteEquipment() {
  if (!selectedId || !window.confirm(t("archive-confirm"))) return;
  try {
    await call("delete_record", { kind: equipKind === "groups" ? "group" : equipKind, id: selectedId });
    selectedId = "";
    equipDraft = false;
    boot = await call("bootstrap");
    render();
  } catch (error) {
    document.getElementById("equip-msg").textContent = String(error);
  }
}

function equipPayload() {
  const id = document.getElementById("record-id").value;
  const body = { id };
  const take = (key) => { const node = document.getElementById(key); if (node) body[key] = node.value; };
  ["label", "make_model", "twist", "twist_hand", "serial", "focal_plane", "drag_model", "bc_reference", "source_note", "bullet_id", "rifle_id", "scope_id", "load_id"].forEach(take);
  ["mag_min", "mag_max", "calibrated_mag", "tracking_factor", "bc", "bc2"].forEach((key) => {
    const node = document.getElementById(key);
    if (node && node.value !== "") body[key] = Number(String(node.value).replace(",", "."));
  });
  ["barrel_length", "sight_height", "weight", "click", "travel_up", "travel_down", "travel_left", "travel_right", "diameter", "length", "mv", "mv_temperature", "sensitivity", "bc2_min", "zero_range", "zero_offset", "zero_powder"].forEach((key) => {
    if (document.getElementById(key)) body[key] = qtyPayload(key);
  });
  if (equipKind === "profile") {
    body.zero_temperature = qtyPayload("zero_temperature");
    body.zero_pressure = qtyPayload("zero_pressure");
    body.zero_pressure_kind = document.getElementById("zero_pressure_kind").value;
    body.zero_humidity = document.getElementById("zero_humidity").value;
    body.zero_altitude = qtyPayload("zero_altitude");
  }
  if (equipKind === "log") {
    ["powder", "powder_lot", "primer", "primer_lot", "case_brand", "case_lot", "bullet_lot", "notes", "load_id"].forEach(take);
    if (document.getElementById("charge")) body.charge = qtyPayload("charge");
    if (document.getElementById("coal")) body.coal = qtyPayload("coal");
    body.chrono_ids = [...document.querySelectorAll(".log-chrono:checked")].map((box) => box.value);
    body.document_ids = [...document.querySelectorAll(".log-doc:checked")].map((box) => box.value);
  }
  return body;
}

async function saveSteps() {
  const range = document.getElementById("range-step");
  const wind = document.getElementById("wind-step");
  if (!range || !wind) return;
  const rangeText = `${range.value} ${range.dataset.unit || "m"}`.trim();
  const windText = `${wind.value} ${wind.dataset.unit || "mps"}`.trim();
  if (!rangeText || !windText) return;
  boot = await call("set_preferences", { body: preferencesBody({ range_step: rangeText, wind_step: windText }) });
}

async function saveSettingsUnits() {
  const custom = {};
  document.querySelectorAll("#settings-units [data-settings-unit]").forEach((select) => {
    custom[select.dataset.settingsUnit] = select.value;
  });
  boot = await call("set_preferences", { body: preferencesBody({ preset: "custom", units: custom, angular: custom.angular }) });
  render();
}

function bindSettings() {
  const about = document.getElementById("open-about-settings");
  if (about) about.addEventListener("click", () => { page = "about"; render(); });
  document.querySelectorAll("[data-appearance]").forEach((button) => button.addEventListener("click", async () => {
    boot = await call("set_preferences", { body: preferencesBody({ appearance: button.dataset.appearance }) });
    applyTheme();
    render();
  }));
  ["range-step", "wind-step"].forEach((id) => {
    const node = document.getElementById(id);
    if (node) node.addEventListener("change", saveSteps);
  });
  document.querySelectorAll("#settings-units [data-settings-unit]").forEach((select) => {
    select.addEventListener("change", saveSettingsUnits);
  });
  document.getElementById("large-text").addEventListener("change", async (event) => {
    boot = await call("set_preferences", { body: preferencesBody({ large_text: event.target.checked }) });
    applyTheme();
  });
  document.getElementById("reduced-motion").addEventListener("change", async (event) => {
    boot = await call("set_preferences", { body: preferencesBody({ reduced_motion: event.target.checked }) });
    applyTheme();
  });
  document.getElementById("weather-save").addEventListener("click", saveWeatherPlace);
  document.getElementById("export").addEventListener("click", () => saveLibrary("json"));
  document.getElementById("export-csv").addEventListener("click", () => saveLibrary("csv"));
  document.getElementById("show-json").addEventListener("click", () => {
    document.getElementById("backup-json").classList.toggle("hidden");
  });
  document.getElementById("restore-file").addEventListener("change", previewLibraryFile);
  document.getElementById("restore-commit").addEventListener("click", () => commitLibrary("replace"));
  document.getElementById("restore-merge").addEventListener("click", () => commitLibrary("merge"));
}

async function saveWeatherPlace() {
  const message = document.getElementById("weather-save-msg");
  try {
    boot = await call("set_preferences", { body: preferencesBody({
      weather_place: document.getElementById("weather-place").value,
      weather_latitude: document.getElementById("weather-lat").value,
      weather_longitude: document.getElementById("weather-lon").value,
      weather_stale_minutes: document.getElementById("weather-stale").value,
    }) });
    if (message) message.textContent = t("weather-saved");
  } catch (error) {
    if (message) message.textContent = explainImport(error);
  }
}

function includeLocation() {
  const box = document.getElementById("include-location");
  return !!(box && box.checked);
}

async function saveLibrary(format) {
  const command = format === "csv" ? "export_library_csv" : "export_library";
  const result = await call(command, { includeLocation: includeLocation() });
  document.getElementById("backup-path").textContent = `${t("backup-wrote")} ${result.path}`;
  document.getElementById("backup-json").textContent = format === "csv" ? result.csv : JSON.stringify(result.document, null, 2);
}

async function previewLibraryFile(event) {
  const file = event.target.files && event.target.files[0];
  const preview = document.getElementById("restore-preview");
  document.getElementById("restore-commit").classList.add("hidden");
  document.getElementById("restore-merge").classList.add("hidden");
  restoreDoc = null;
  if (!file) return;
  const text = await file.text();
  const csv = file.name.toLowerCase().endsWith(".csv") || text.trimStart().startsWith("format,schema_version");
  try {
    restoreDoc = csv ? { csv: text } : JSON.parse(text);
    restorePreview = await call("import_library", { document: restoreDoc, commit: false, policy: "replace" });
    preview.textContent = libraryPreviewText(restorePreview);
    document.getElementById("restore-commit").classList.remove("hidden");
    document.getElementById("restore-merge").classList.remove("hidden");
  } catch (error) {
    restoreDoc = null;
    preview.textContent = explainImport(error);
  }
}

async function commitLibrary(policy) {
  if (!restoreDoc) return;
  try {
    await call("import_library", { document: restoreDoc, commit: true, policy });
    restoreDoc = null;
    boot = await call("bootstrap");
    render();
  } catch (error) {
    const preview = document.getElementById("restore-preview");
    if (preview) preview.textContent = explainImport(error);
  }
}

function syncHtml() {
  const exportOn = syncTab === "export";
  const importOn = syncTab === "import";
  const facts = bundlePreview
    ? `<dl class="ho-kv mini"><dt>${esc(t("sync-schema"))}</dt><dd>${esc(bundlePreview.schema_version)}</dd><span class="ho-unit"></span><dt>${esc(t("sync-local-only"))}</dt><dd>${esc(bundlePreview.local_only || 0)}</dd><span class="ho-unit"></span></dl>`
    : "";
  const rail = `<h2>${esc(t("sync-bundle"))}</h2>
    <p class="ho-hint">${esc(t("sync-intro"))}</p>
    <div class="ho-seg" id="sync-tabs" role="tablist" aria-label="${esc(t("sync-tabs"))}">
      ${syncModeButton("export", "sync-export", exportOn)}
      ${syncModeButton("import", "sync-import", importOn)}
    </div>
    <div role="tabpanel" data-sync-panel="export" ${exportOn ? "" : "hidden"}>
      <p class="ho-hint">${esc(t("sync-export-intro"))}</p>
      <label class="ho-check"><input type="checkbox" id="bundle-include" title="${esc(t("sync-include-tip"))}" ${bundleInclude ? "checked" : ""} /> ${esc(t("include-location"))}</label>
      <button type="button" class="ho-btn primary" id="bundle-export" title="${esc(t("sync-export-tip"))}">${esc(t("sync-export-button"))}</button>
    </div>
    <div role="tabpanel" data-sync-panel="import" ${importOn ? "" : "hidden"}>
      ${syncImportControls()}
    </div>
    ${facts}
    <p id="bundle-message" class="${bundleError ? "error" : "meta"}">${esc(bundleMessage)}</p>
    ${bundleDetail ? `<p id="bundle-detail" class="meta">${esc(bundleDetail)}</p>` : ""}`;
  const main = importOn ? syncImportTable() : `<p class="ho-hint">${esc(t("sync-export-intro"))}</p>`;
  return frame({ rail, main, body: "with-list", railLabel: t("nav-sync") });
}

function syncModeButton(id, key, on) {
  return `<button type="button" role="tab" data-sync-tab="${id}" aria-selected="${on}" tabindex="${on ? 0 : -1}">${esc(t(key))}</button>`;
}

function syncImportControls() {
  return `<p class="ho-hint">${esc(t("sync-import-intro"))}</p>
    <p class="ho-hint">${esc(t("sync-import-privacy"))}</p>
    <div class="ho-field"><div class="head"><label for="bundle-file">${esc(t("sync-file"))}</label></div>
      <input id="bundle-file" type="file" accept=".holdover,application/zip" title="${esc(t("sync-file-tip"))}" />
    </div>`;
}

function syncImportTable() {
  if (!bundlePreview) return `<p class="ho-hint">${esc(t("sync-import-intro"))}</p>`;
  const rows = bundlePreview.rows || [];
  const shown = syncVisibleRows(rows);
  const hidden = syncFilteredRows(rows).length - shown.length;
  const more = hidden > 0 ? `<p class="ho-hint">${esc(t("sync-more").replace("{count}", String(hidden)))}</p>` : "";
  const note = bundleConflictsOpen()
    ? `<p class="ho-hint">${esc(t("sync-unresolved-note"))}</p>`
    : (bundleHasWork() ? "" : `<p class="ho-hint">${esc(t("sync-nothing"))}</p>`);
  const body = shown.map(syncRowHtml).join("") || `<tr><td class="text" colspan="4">${esc(t("sync-filter-empty"))}</td></tr>`;
  return `<div class="filters" id="sync-filters" role="tablist" aria-label="${esc(t("sync-filter"))}">
      ${["conflict", "new", "updated", "deleted", "unchanged", "all"].map(syncFilterButton).join("")}
    </div>
    <section class="ho-card flush"><div class="ho-table-wrap"><table class="ho-table"><thead><tr>
      <th class="text">${esc(t("archive-kind"))}</th><th class="text">${esc(t("sync-filter"))}</th><th class="text">${esc(t("label"))}</th><th class="text"></th>
    </tr></thead><tbody>${body}</tbody></table></div></section>
    ${more}
    ${note}
    <button type="button" class="ho-btn primary" id="bundle-apply" title="${esc(t("sync-apply-tip"))}" ${bundleCanApply() ? "" : "disabled"}>${esc(t("sync-apply"))}</button>`;
}

function syncFilterButton(id) {
  const counts = (bundlePreview && bundlePreview.counts) || {};
  const rows = (bundlePreview && bundlePreview.rows) || [];
  const count = id === "all" ? rows.length : (counts[id] || 0);
  const on = syncFilter === id;
  const label = id === "all" ? t("sync-filter-all") : t("sync-status-" + id);
  return `<button type="button" class="ho-btn" role="tab" data-sync-filter="${id}" aria-selected="${on}" tabindex="${on ? 0 : -1}">${esc(label)} <span class="ho-badge">${count}</span></button>`;
}

function syncFilteredRows(rows) {
  if (syncFilter === "all") return rows;
  return rows.filter((row) => row.status === syncFilter);
}

function syncVisibleRows(rows) {
  return syncFilteredRows(rows).slice(0, 80);
}

function syncRowHtml(row) {
  const choices = (row.choices || []).map((choice) => {
    const pressed = bundleChoices[row.id] === choice;
    return `<button type="button" data-bundle-choice="${esc(choice)}" data-id="${esc(row.id)}" aria-pressed="${pressed}" title="${esc(t("sync-choice-tip"))}">${esc(choiceLabel(choice))}</button>`;
  }).join("");
  const actions = choices ? `<span class="ho-seg">${choices}</span>` : "";
  return `<tr>
    <td class="text">${esc(kindLabel(row.kind))}</td>
    <td class="text">${esc(statusLabel(row.status))}</td>
    <td class="text">${syncNames(row)}</td>
    <td class="text">${actions}</td>
  </tr>`;
}

function syncNames(row) {
  const local = row.local_label || "";
  const remote = row.remote_label || row.label || "";
  if (row.status === "new") return `<span>${esc(remote || row.label || "")}</span>`;
  if (local && remote && local !== remote) {
    return `<span>${esc(t("sync-local"))}: ${esc(local)}</span><span>${esc(t("sync-remote"))}: ${esc(remote)}</span>`;
  }
  return `<span>${esc(row.label || local || remote)}</span>`;
}

function statusLabel(status) {
  const key = "sync-status-" + status;
  const text = t(key);
  return text === key ? status : text;
}

function choiceLabel(choice) {
  const keys = { "keep-mine": "sync-keep-mine", "take-theirs": "sync-take-theirs", "keep-both": "sync-keep-both" };
  return t(keys[choice] || choice);
}

function bundleConflictsOpen() {
  const rows = (bundlePreview && bundlePreview.rows) || [];
  return rows.some((row) => row.status === "conflict" && !bundleChoices[row.id]);
}

function bundleHasWork() {
  const counts = (bundlePreview && bundlePreview.counts) || {};
  return ["new", "updated", "deleted", "conflict"].some((key) => Number(counts[key] || 0) > 0);
}

function bundleCanApply() {
  return !!(bundlePreview && bundleBytes && bundleHasWork() && !bundleConflictsOpen());
}

function errorText(error) {
  if (typeof error === "string") return error;
  if (error && typeof error.message === "string" && error.message) return error.message;
  try {
    const text = JSON.stringify(error);
    return text == null ? String(error) : text;
  } catch (ignored) {
    return String(error);
  }
}

function explainBundle(error) {
  const text = errorText(error);
  let message = "";
  if (text.includes("bundle-schema-newer")) message = t("sync-schema-newer");
  else if (text.includes("bundle-schema-older")) message = t("sync-schema-older");
  else if (text.includes("bundle-foreign")) message = t("sync-foreign");
  else if (text.includes("bundle-truncated")) message = t("sync-truncated");
  else if (text.includes("bundle-corrupt")) message = t("sync-corrupt");
  else if (text.includes("bundle-unresolved")) message = t("sync-unresolved");
  else if (text.includes("bundle-keep-both")) message = t("sync-keep-both-refused");
  else if (text.includes("bundle-missing-pdf")) message = t("sync-missing-pdf");
  else if (text.includes("bundle-duplicate")) message = t("sync-duplicate");
  else if (text.includes("bundle-bad")) message = t("sync-bad");
  if (message) return { message, detail: "" };
  return { message: t("sync-failed"), detail: text && text !== t("sync-failed") ? text : "" };
}

function bindSync() {
  bindTablist("#sync-tabs", "syncTab", (id) => {
    syncTab = id;
    render();
    const next = document.querySelector(`#sync-tabs [data-sync-tab="${id}"]`);
    if (next) next.focus();
  });
  bindTablist("#sync-filters", "syncFilter", (id) => {
    syncFilter = id;
    render();
    const next = document.querySelector(`#sync-filters [data-sync-filter="${id}"]`);
    if (next) next.focus();
  });
  const include = document.getElementById("bundle-include");
  if (include) include.addEventListener("change", () => { bundleInclude = include.checked; });
  const exportButton = document.getElementById("bundle-export");
  if (exportButton) exportButton.addEventListener("click", exportBundle);
  const file = document.getElementById("bundle-file");
  if (file) file.addEventListener("change", previewBundleFile);
  document.querySelectorAll("[data-bundle-choice]").forEach((button) => {
    button.addEventListener("click", () => {
      const id = button.dataset.id;
      bundleChoices[id] = button.dataset.bundleChoice;
      render();
      const next = document.querySelector(`[data-bundle-choice="${bundleChoices[id]}"][data-id="${CSS.escape(id)}"]`);
      if (next) next.focus();
    });
  });
  const apply = document.getElementById("bundle-apply");
  if (apply) apply.addEventListener("click", applyBundle);
}

function bindTablist(selector, datasetKey, onSelect) {
  const list = document.querySelector(selector);
  if (!list) return;
  const tabs = [...list.querySelectorAll("[role=tab]")];
  tabs.forEach((button) => {
    button.addEventListener("click", () => onSelect(button.dataset[datasetKey]));
    button.addEventListener("keydown", (event) => {
      const index = tabs.indexOf(button);
      let next = -1;
      if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
      else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = tabs.length - 1;
      if (next < 0) return;
      event.preventDefault();
      event.stopPropagation();
      onSelect(tabs[next].dataset[datasetKey]);
    });
  });
}

function showBundleStatus() {
  const node = document.getElementById("bundle-message");
  if (node) {
    node.className = bundleError ? "error" : "meta";
    node.textContent = bundleMessage;
  }
  let detail = document.getElementById("bundle-detail");
  if (!bundleDetail) {
    if (detail) detail.remove();
    return;
  }
  if (!detail && node) {
    detail = document.createElement("p");
    detail.id = "bundle-detail";
    detail.className = "meta";
    node.insertAdjacentElement("afterend", detail);
  }
  if (detail) detail.textContent = bundleDetail;
}

function setBundleError(error) {
  const explained = explainBundle(error);
  bundleError = true;
  bundleMessage = explained.message;
  bundleDetail = explained.detail;
}

async function exportBundle() {
  bundleError = false;
  bundleDetail = "";
  try {
    const result = await call("export_bundle", { includeLocation: bundleInclude });
    bundleMessage = `${t("sync-wrote")} ${result.path}`;
  } catch (error) {
    setBundleError(error);
  }
  showBundleStatus();
}

async function previewBundleFile(event) {
  const file = event.target.files && event.target.files[0];
  if (!file) return;
  bundleError = false;
  bundleDetail = "";
  bundleMessage = t("sync-reading");
  showBundleStatus();
  try {
    bundleBytes = await readFileBase64(file);
    bundleChoices = {};
    bundlePreview = await call("preview_bundle", { bytesBase64: bundleBytes });
    bundleMessage = "";
    const conflicts = bundlePreview.counts && Number(bundlePreview.counts.conflict || 0);
    syncFilter = conflicts ? "conflict" : "all";
  } catch (error) {
    bundlePreview = null;
    bundleBytes = "";
    bundleChoices = {};
    setBundleError(error);
  }
  render();
}

async function applyBundle() {
  if (!bundleCanApply()) return;
  bundleError = false;
  bundleDetail = "";
  try {
    await call("import_bundle", { bytesBase64: bundleBytes, resolutions: bundleChoices });
    bundlePreview = null;
    bundleBytes = "";
    bundleChoices = {};
    bundleMessage = t("sync-applied");
    boot = await call("bootstrap");
    render();
  } catch (error) {
    setBundleError(error);
    render();
  }
}

function readFileBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const text = String(reader.result || "");
      const comma = text.indexOf(",");
      resolve(comma >= 0 ? text.slice(comma + 1) : text);
    };
    reader.onerror = () => reject(reader.error || new Error("bundle-corrupt"));
    reader.readAsDataURL(file);
  });
}

function preferencesBody(patch) {
  const s = boot.settings;
  return Object.assign({
    preset: s.preset,
    units: s.units,
    angular: s.angular,
    appearance: s.appearance,
    large_text: s.large_text,
    reduced_motion: s.reduced_motion,
    card_pinned: s.card_pinned,
  }, patch);
}

async function setPreset(preset) {
  const body = preferencesBody({ preset });
  body.units = Object.assign({}, boot.settings.units);
  if (preset !== "custom") delete body.angular;
  boot = await call("set_preferences", { body });
  render();
}

function cycleChip(chip) {
  const input = document.getElementById(chip.dataset.for);
  if (!input) return;
  const cycle = CYCLES[input.dataset.dim] || CYCLES.length;
  let index = cycle.indexOf(input.dataset.unit);
  if (index < 0) index = 0;
  const next = cycle[(index + 1) % cycle.length];
  let si = Number(input.dataset.si);
  if (!Number.isFinite(si)) {
    const parsed = parseLocal(input.dataset.text || input.value, input.dataset.dim, input.dataset.unit);
    si = parsed ? parsed.si : NaN;
  }
  input.dataset.unit = next;
  chip.textContent = shownUnit(next);
  if (Number.isFinite(si)) {
    input.dataset.si = String(si);
    input.value = formatLocal(si, input.dataset.dim, next);
    input.dataset.text = input.value;
    input.dataset.dirty = "1";
  }
  if (input.id === "target_speed") {
    moving.speedUnit = next;
    moving.speedText = input.value;
  }
  showPreview(input);
  if (page === "solve") scheduleSolve(0);
  if (page === "reticle") scheduleReticle();
  if (page === "charts") {
    const ids = [...document.querySelectorAll(".overlay:checked")].map((box) => box.value).slice(0, 6);
    buildImpact(ids);
  }
  if (page === "targets") paintTarget();
  if (page === "compare") {
    rememberCompareNumbers();
    scheduleCompare(0);
  }
  if (page === "groups") rememberGroupFields();
}

function showPreview(input) {
  const preview = document.getElementById(`${input.id}-preview`);
  if (!preview) return;
  const field = input.closest(".ho-field");
  if (field && field.classList.contains("invalid")) return;
  const parsed = input.dataset.dim ? parseLocal(input.value, input.dataset.dim, input.dataset.unit) : null;
  if (!parsed || parsed.bare) {
    preview.textContent = preview.dataset.base || "";
    return;
  }
  const shown = formatLocal(parsed.si, input.dataset.dim, input.dataset.unit);
  preview.textContent = `${input.value.trim()} ${t("preview-arrow")} ${shown} ${shownUnit(input.dataset.unit)}`;
}

function qtyPayload(id) {
  const input = document.getElementById(id);
  if (!input) return null;
  if (input.dataset.dirty === "1") return { text: input.value, unit: input.dataset.unit };
  const text = input.dataset.text || input.value;
  return { text, unit: input.dataset.unit };
}

function storedQuantity(text, unit) {
  return { text: text || "", unit: unit || "" };
}

function airPayload(name, fallbackUnit) {
  const edited = qtyPayload(name);
  if (edited) return edited;
  const air = boot.settings.current_air || {};
  const node = air[name];
  if (node && typeof node === "object" && node.text) return { text: node.text, unit: node.unit || fallbackUnit };
  if (typeof node === "string" && node) return { text: node, unit: fallbackUnit };
  return storedQuantity("", fallbackUnit);
}

function solveBody() {
  const profile = document.getElementById("profile");
  const s = boot.settings;
  const air = s.current_air || {};
  const body = {
    profile_id: profile && profile.value ? profile.value : s.active_profile_id,
    range: qtyPayload("range") || storedQuantity(s.last_range, units().length),
    wind_speed: qtyPayload("wind_speed") || storedQuantity(s.last_wind_speed, units().velocity),
    wind_direction: (document.getElementById("wind_direction") || {}).value || s.last_wind_direction || "12oc",
    inclination: qtyPayload("inclination") || storedQuantity(s.last_inclination, "deg"),
    cant: qtyPayload("cant") || { text: "0 deg", unit: "deg" },
    latitude: qtyPayload("latitude") || { text: "0 deg", unit: "deg" },
    azimuth: qtyPayload("azimuth") || { text: "0 deg", unit: "deg" },
    current_air: {
      temperature: airPayload("temperature", units().temperature),
      pressure: airPayload("pressure", units().pressure),
      pressure_kind: (document.getElementById("pressure_kind") || {}).value || air.pressure_kind || "station",
      humidity: (document.getElementById("humidity") || {}).value || (typeof air.humidity === "string" ? air.humidity : (air.humidity_text || "0%")),
      altitude: airPayload("altitude", units().length),
    },
    effects: document.querySelector("[data-effect]") ? effects() : (s.effects || effects()),
    target_speed: liveTargetSpeed(),
    target_direction: liveTargetDirection(),
    reaction_s: liveReaction(),
    lead_target: leadTarget,
    facing: leadFacing,
    angular: s.angular || "mrad",
    range_step: storedQuantity(s.range_step || "10 m", units().length),
    wind_step: storedQuantity(s.wind_step || "1 mps", units().velocity),
  };
  if (!body.target_speed) delete body.target_speed;
  return body;
}

function liveTargetSpeed() {
  const input = document.getElementById("target_speed");
  if (input && String(input.value || "").trim()) {
    moving.speedText = input.value;
    moving.speedUnit = input.dataset.unit || moving.speedUnit;
    return qtyPayload("target_speed");
  }
  if (moving.speedText && String(moving.speedText).trim()) {
    return { text: moving.speedText, unit: moving.speedUnit || units().velocity };
  }
  return undefined;
}

function liveTargetDirection() {
  const input = document.getElementById("target_direction");
  if (input && String(input.value || "").trim()) {
    moving.direction = input.value;
    return input.value;
  }
  return moving.direction || "3oc";
}

function liveReaction() {
  const input = document.getElementById("reaction_s");
  const text = input ? input.value : moving.reaction;
  if (input) moving.reaction = input.value;
  const value = Number(String(text || "0").replace(",", "."));
  return Number.isFinite(value) ? value : 0;
}

function effects() {
  const flags = { incline: true, spin_drift: true, coriolis: false, cant: false, aerodynamic_jump: false };
  document.querySelectorAll("[data-effect]").forEach((box) => { flags[box.dataset.effect] = box.checked; });
  return flags;
}

function scheduleSolve(delay) {
  window.clearTimeout(solveTimer);
  solveTimer = window.setTimeout(runSolve, delay);
}

async function runSolve() {
  const hold = document.getElementById("hold");
  if (!hold || page !== "solve") return;
  const bad = ["range", "wind_speed", "wind_direction", "inclination", "temperature", "pressure", "humidity", "altitude", "target_speed", "latitude", "azimuth", "cant"]
    .map((id) => document.getElementById(id))
    .find((input) => input && fieldIsInvalid(input));
  if (bad) {
    markInvalid(bad, true);
    stale = true;
    solveError = staleSentence(bad);
    paintSolve();
    return;
  }
  try {
    const body = solveBody();
    if (!body.profile_id) {
      stale = true;
      solveError = t("no-profile");
      paintSolve();
      return;
    }
    lastSolve = await call("solve", { body });
    stale = false;
    solveError = "";
    rememberGood();
    const leadHost = document.getElementById("lead-result");
    if (leadHost) leadHost.innerHTML = leadHold();
    paintLeadFace();
    paintSolve();
  } catch (err) {
    stale = true;
    solveError = `${t("last-valid")} ${err}`;
    paintSolve();
  }
}

function parseLocal(text, dim, unit) {
  const raw = String(text || "").trim().replace("−", "-");
  if (!raw) return null;
  const match = raw.match(/^([+-]?(?:\d+(?:[.,]\d+)?|\d*[.,]\d+))\s*(.*)$/);
  if (!match) return null;
  const value = Number(match[1].replace(",", "."));
  if (!Number.isFinite(value)) return null;
  const suffix = (match[2] || "").replace(/[°·\s]/g, "").toLowerCase();
  const si = toSi(value, dim, suffix || unit);
  if (!Number.isFinite(si)) return null;
  return { si, bare: !suffix };
}

function toSi(value, dim, unit) {
  const u = String(unit || "").toLowerCase();
  if (dim === "temperature") {
    if (u === "f") return (value - 32) * 5 / 9;
    if (u === "k") return value - 273.15;
    if (u === "c" || u === "") return value;
    return NaN;
  }
  const factors = {
    m: 1, mm: 0.001, cm: 0.01, in: 0.0254, ft: 0.3048, yd: 0.9144,
    g: 0.001, kg: 1, gr: 0.00006479891, lb: 0.45359237, oz: 0.028349523125,
    mps: 1, "m/s": 1, fps: 0.3048, mph: 0.44704, "km/h": 1000 / 3600, kn: 1852 / 3600,
    hpa: 100, mbar: 100, inhg: 3386.389, mmhg: 3386.389 / 25.4, psi: 6894.757293168361, pa: 1,
    mrad: 0.001, mil: 0.001, moa: Math.PI / (180 * 60), deg: Math.PI / 180,
    iphy: 0.001 / 3.6, "cm@100m": 0.0001, "cm/100m": 0.0001,
    j: 1, ftlb: 0.3048 * 0.45359237 * 9.80665,
  };
  if (!Object.prototype.hasOwnProperty.call(factors, u)) return NaN;
  return value * factors[u];
}

function formatDelta(si, dim, unit) {
  if (dim === "temperature") {
    const shown = String(unit || "").toLowerCase() === "f" ? si * 9 / 5 : si;
    return String(Math.round(shown * 10000) / 10000);
  }
  return formatLocal(si, dim, unit);
}

function formatLocal(si, dim, unit) {
  let shown = si;
  const u = String(unit || "").toLowerCase();
  if (dim === "temperature") {
    if (u === "f") shown = si * 9 / 5 + 32;
    else if (u === "k") shown = si + 273.15;
  } else {
    const back = toSi(1, dim, unit);
    if (back) shown = si / back;
  }
  return String(Math.round(shown * 10000) / 10000);
}

document.addEventListener("keydown", (event) => {
  if (!boot || !boot.settings || !boot.settings.onboarded) return;
  if (event.key === "Escape" && (paletteOpen || unitsOpen)) {
    event.preventDefault();
    paletteOpen = false;
    unitsOpen = false;
    render();
    return;
  }
  const typing = event.target && (event.target.matches("input, textarea, select"));
  if (event.ctrlKey && event.key.toLowerCase() === "u") {
    event.preventDefault();
    const index = PRESETS.indexOf(boot.settings.preset);
    setPreset(PRESETS[(index + 1) % PRESETS.length]);
    return;
  }
  if (event.ctrlKey && event.key.toLowerCase() === "k") {
    event.preventDefault();
    paletteOpen = !paletteOpen;
    render();
    if (paletteOpen) fillPalette();
    return;
  }
  if (event.ctrlKey && event.key.toLowerCase() === "p") {
    event.preventDefault();
    const profile = document.getElementById("profile");
    if (profile) profile.focus();
    return;
  }
  if (event.ctrlKey && event.shiftKey && event.key.toLowerCase() === "c") {
    event.preventDefault();
    copyHold();
    return;
  }
  if (event.key === "F11") {
    event.preventDefault();
    fieldMode = !fieldMode;
    applyTheme();
    return;
  }
  if (typing || page !== "solve" || !boot || !boot.settings.onboarded) return;
  const key = event.key.toLowerCase();
  if (key === "r") { event.preventDefault(); focusId("range"); }
  else if (key === "w") { event.preventDefault(); focusId("wind_speed"); }
  else if (key === "d") { event.preventDefault(); focusId("wind_direction"); }
  else if (key === "a") { event.preventDefault(); focusId("inclination"); }
  else if (key === "u") cycleAngular();
  else if (event.key === "ArrowUp" || event.key === "ArrowDown") { event.preventDefault(); stepRange(event.key === "ArrowUp" ? 1 : -1, event); }
  else if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); stepClock(event.key === "ArrowRight" ? 1 : -1); }
  else if (event.key === "+" || event.key === "=") { event.preventDefault(); stepWind(1); }
  else if (event.key === "-" || event.key === "_") { event.preventDefault(); stepWind(-1); }
});

function focusId(id) {
  const node = document.getElementById(id);
  if (!node) return;
  node.focus();
  if (typeof node.select === "function") node.select();
}

async function setAngular(angular) {
  boot = await call("set_preferences", { body: preferencesBody({ angular }) });
  if (lastSolve) lastSolve.angular = angular;
  scheduleSolve(0);
}

async function cycleAngular() {
  const index = ANGULAR.indexOf(boot.settings.angular || "mrad");
  await setAngular(ANGULAR[(index + 1) % ANGULAR.length]);
}

function stepRange(sign, event) {
  const input = document.getElementById("range");
  if (!input) return;
  const parsed = parseLocal(input.value, "length", input.dataset.unit);
  if (!parsed) {
    markInvalid(input, true);
    stale = true;
    solveError = staleSentence(input);
    paintSolve();
    return;
  }
  let factor = 1;
  if (event.shiftKey) factor *= 4;
  if (event.ctrlKey) factor *= 0.1;
  const step = (parseLocal(boot.settings.range_step, "length", "m") || { si: 10 }).si * factor * sign;
  const next = Math.max(1, parsed.si + step);
  input.value = formatLocal(next, "length", input.dataset.unit);
  input.dataset.dirty = "1";
  input.dataset.si = String(next);
  input.dataset.text = `${input.value} ${input.dataset.unit}`;
  markInvalid(input, false);
  scheduleSolve(0);
}

function stepClock(sign) {
  const input = document.getElementById("wind_direction");
  if (!input) return;
  const match = String(input.value).match(/(\d+)/);
  let hour = match ? Number(match[1]) : 12;
  hour = (hour + sign + 12) % 12;
  if (hour === 0) hour = 12;
  input.value = clockPhrase(String(hour));
  syncWindClock(hour);
  scheduleSolve(0);
}

function stepWind(sign) {
  const input = document.getElementById("wind_speed");
  if (!input) return;
  const parsed = parseLocal(input.value, "velocity", input.dataset.unit);
  if (!parsed) {
    markInvalid(input, true);
    stale = true;
    solveError = staleSentence(input);
    paintSolve();
    return;
  }
  const windStep = (parseLocal(boot.settings.wind_step, "velocity", "mps") || { si: 1 }).si;
  const next = Math.max(0, parsed.si + windStep * sign);
  input.value = formatLocal(next, "velocity", input.dataset.unit);
  input.dataset.dirty = "1";
  input.dataset.si = String(next);
  input.dataset.text = `${input.value} ${input.dataset.unit}`;
  markInvalid(input, false);
  scheduleSolve(0);
}

async function copyHold() {
  if (!lastSolve) return;
  const text = `${lastSolve.elevation.direction} ${lastSolve.elevation.primary} · ${lastSolve.windage.direction} ${lastSolve.windage.primary}`;
  try {
    await navigator.clipboard.writeText(text);
  } catch (error) {
    const node = document.getElementById("solve-error");
    if (node) {
      node.hidden = false;
      node.innerHTML = `<p class="ho-reason">${esc(t("copy-failed"))}</p>`;
    }
  }
}

function paletteRows() {
  const screens = [
    ["solve", t("nav-solve"), ""],
    ["card", t("nav-card"), ""],
    ["charts", t("nav-charts"), ""],
    ["reticle", t("nav-reticle"), ""],
    ["targets", t("nav-targets"), ""],
    ["groups", t("nav-groups"), ""],
    ["pbr", t("nav-pbr"), ""],
    ["truing", t("nav-truing"), ""],
    ["equipment", t("nav-equipment"), ""],
    ["compare", t("nav-compare"), ""],
    ["documents", t("nav-documents"), ""],
    ["sync", t("nav-sync"), ""],
    ["about", t("nav-about"), ""],
    ["settings", t("nav-settings"), ""],
  ];
  const commands = [
    ["metric", t("command-metric"), ""],
    ["us", t("command-us"), ""],
    ["custom", t("command-custom"), "Ctrl+U"],
    ["field", t("command-field"), "F11"],
    ["copy", t("command-copy"), "Ctrl+Shift+C"],
  ];
  const q = paletteQuery.trim().toLowerCase();
  const keep = (row) => !q || row[1].toLowerCase().includes(q);
  return { screens: screens.filter(keep), commands: commands.filter(keep) };
}

function paletteMark(label) {
  const query = paletteQuery.trim();
  if (!query) return esc(label);
  const at = label.toLowerCase().indexOf(query.toLowerCase());
  if (at < 0) return esc(label);
  return `${esc(label.slice(0, at))}<mark>${esc(label.slice(at, at + query.length))}</mark>${esc(label.slice(at + query.length))}`;
}

function fillPalette() {
  const box = document.getElementById("palette");
  if (!box) return;
  const { screens, commands } = paletteRows();
  const flat = screens.map((row) => row.concat([t("palette-group-screens")])).concat(commands.map((row) => row.concat([t("palette-group-commands")])));
  if (paletteIndex >= flat.length) paletteIndex = 0;
  const item = (row, index) => {
    const keys = row[2] ? row[2].split("+").map((part) => `<kbd>${esc(part)}</kbd>`).join("") : "";
    return `<li role="option" data-cmd="${esc(row[0])}" aria-selected="${index === paletteIndex}"><span class="grp">${esc(row[3])}</span><span>${paletteMark(row[1])}</span><span class="sp"></span>${keys}</li>`;
  };
  let html = "";
  let index = 0;
  if (screens.length) {
    html += `<li class="sec" role="presentation">${esc(t("palette-group-screens"))}</li>`;
    screens.forEach((row) => { html += item(row.concat([t("palette-group-screens")]), index); index += 1; });
  }
  if (commands.length) {
    html += `<li class="sec" role="presentation">${esc(t("palette-group-commands"))}</li>`;
    commands.forEach((row) => { html += item(row.concat([t("palette-group-commands")]), index); index += 1; });
  }
  box.innerHTML = `<div class="ho-dialog ho-palette">
      <div class="q"><input id="palette-q" type="search" role="combobox" aria-expanded="true" aria-controls="palette-list" aria-autocomplete="list" placeholder="${esc(t("palette-search"))}" aria-label="${esc(t("palette-search"))}" value="${esc(paletteQuery)}" autocomplete="off" spellcheck="false" /></div>
      <ul id="palette-list" role="listbox" aria-label="${esc(t("commands"))}">${html || `<li class="sec">${esc(t("sync-filter-empty"))}</li>`}</ul>
      <footer><span class="ho-cap"><kbd>↑</kbd><kbd>↓</kbd> ${esc(t("palette-move"))} · <kbd>Enter</kbd> ${esc(t("palette-open"))} · <kbd>Esc</kbd> ${esc(t("palette-close"))}</span></footer>
    </div>`;
  const input = document.getElementById("palette-q");
  if (input) {
    input.focus();
    const end = input.value.length;
    try { input.setSelectionRange(end, end); } catch (error) { /* search inputs can refuse a range */ }
    input.addEventListener("input", () => {
      paletteQuery = input.value;
      paletteIndex = 0;
      fillPalette();
    });
    input.addEventListener("keydown", (event) => {
      const count = flat.length;
      if (!count) return;
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        event.stopPropagation();
        paletteIndex = event.key === "ArrowDown" ? (paletteIndex + 1) % count : (paletteIndex + count - 1) % count;
        fillPalette();
      } else if (event.key === "Enter") {
        event.preventDefault();
        const chosen = flat[paletteIndex];
        if (chosen) runCommand(chosen[0]);
      }
    });
  }
  box.querySelectorAll("[data-cmd]").forEach((button) => button.addEventListener("click", () => runCommand(button.dataset.cmd)));
}

function runCommand(id) {
  paletteOpen = false;
  if (PRESETS.includes(id)) setPreset(id);
  else if (id === "field") { fieldMode = !fieldMode; applyTheme(); render(); }
  else if (id === "copy") { copyHold(); render(); }
  else { page = id; render(); }
}

function activeProfile() {
  const id = (document.getElementById("profile") && document.getElementById("profile").value) || boot.settings.active_profile_id;
  return (boot.equipment.profiles || []).find((item) => item.id === id) || null;
}

function applyCardPage(data) {
  if (!data || !data.page) return;
  let style = document.getElementById("card-page-style");
  if (!style) {
    style = document.createElement("style");
    style.id = "card-page-style";
    document.head.appendChild(style);
  }
  const width = Number(data.page.width_mm);
  const height = Number(data.page.height_mm);
  style.textContent = data.print_css || `@media print { @page { size: ${width}mm ${height}mm; margin: 10mm; } }`;
}

function downloadText(name, text, type) {
  const blob = new Blob([text], { type });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
}

function downloadSvg(name) {
  const host = document.getElementById("chart-body") || document.getElementById("impact-body");
  if (!host) return;
  const clone = host.cloneNode(true);
  clone.querySelectorAll(".noprint").forEach((node) => node.remove());
  downloadText(name, `<svg xmlns="http://www.w3.org/2000/svg">${clone.innerHTML}</svg>`, "image/svg+xml");
}

function downloadPng(name, selector) {
  const svg = document.querySelector(selector || "#chart-body svg");
  if (!svg) return;
  const xml = new XMLSerializer().serializeToString(svg);
  const blob = new Blob([xml], { type: "image/svg+xml" });
  const url = URL.createObjectURL(blob);
  const image = new Image();
  image.onload = () => {
    const canvas = document.createElement("canvas");
    const box = svg.viewBox.baseVal;
    canvas.width = Math.max(1, box.width || svg.clientWidth || 800);
    canvas.height = Math.max(1, box.height || svg.clientHeight || 400);
    const context = canvas.getContext("2d");
    context.fillStyle = "#ffffff";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    URL.revokeObjectURL(url);
    const link = document.createElement("a");
    link.href = canvas.toDataURL("image/png");
    link.download = name;
    link.click();
  };
  image.src = url;
}

function chartCsv() {
  const series = chartData.series || [];
  const header = ["Range"];
  series.forEach((item) => {
    ["path", "windage", "velocity", "energy", "tof", "mach"].forEach((key) => header.push(`${item.label} ${key}`));
  });
  const count = series[0] && series[0].points ? series[0].points.length : 0;
  const lines = [header.join(",")];
  for (let index = 0; index < count; index += 1) {
    const cells = [series[0].points[index].range];
    series.forEach((item) => {
      const point = item.points[index] || {};
      cells.push(point.path, point.windage, point.velocity, point.energy, point.tof, point.mach);
    });
    lines.push(cells.join(","));
  }
  return lines.join("\n");
}

async function buildImpact(ids) {
  const host = document.getElementById("impact-body");
  if (!host) return;
  try {
    const body = solveBody();
    body.profile_ids = ids;
    body.range = qtyPayload("impact-range") || body.range;
    body.target = document.getElementById("impact-target").value;
    body.facing = (document.getElementById("impact-facing") || {}).value || impactFacing;
    impactTarget = body.target;
    impactFacing = body.facing;
    body.mv_sd = qtyPayload("mv_sd");
    body.range_error = qtyPayload("range_error");
    body.brackets = document.getElementById("impact-brackets").value;
    if (body.target === "custom") body.zone = qtyPayload("impact-zone");
    const groupPick = document.getElementById("impact-group");
    if (groupPick) impactGroupId = groupPick.value;
    if (impactGroupId) body.group_id = impactGroupId;
    impactData = await call("impact", { body });
    host.innerHTML = impactView(impactData);
  } catch (error) {
    host.innerHTML = `<p class="error">${esc(error)}</p>`;
  }
}

function impactView(data) {
  const spec = data.geometry && data.geometry.paths ? data.geometry : {
    rings: (data.rings_cm || []).map((diameter) => ({ diameter_cm: diameter, label_primary: "", verified: true })),
    paths: [],
    flags: [],
  };
  const pois = data.pois || [];
  const picture = faceSvg(spec, {
    pois,
    ringing: impactRinging,
    showSilhouette: impactSilhouette,
    measured: data.measured || [],
    label: t("impact-title"),
  });
  const badges = flagHtml(spec.flags);
  const hint = data.target === "custom" ? t("impact-custom-note") : (spec.subtitle || data.note || "");
  return `${badges}
    <p class="hint">${esc(hint)}</p>
    <p class="axis-title">${esc(t("impact-scale"))}</p>
    ${picture}
    <p class="legend">${pois.map((poi, index) => `<span><i class="swatch swatch-${index % 6}"></i>${esc(poi.label)} · ${esc(t("impact-poi"))}</span>`).join("")}</p>
    <p class="hint">${esc(t("impact-bars"))}</p>
    <div class="row">
      <button type="button" id="impact-svg" title="${esc(t("impact-svg-tip"))}">${esc(t("impact-save-svg"))}</button>
      <button type="button" id="impact-png" title="${esc(t("impact-png-tip"))}">${esc(t("impact-save-png"))}</button>
    </div>`;
}

function pbrHtml() {
  const zero = pbrMode === "fixed"
    ? qtyField("pbr-zero", t("pbr-zero-input"), pbrZeroText, units().length, "length", null, null, "pbr-zero")
    : `<div class="ho-field"><div class="head"><label for="pbr-zero">${esc(t("pbr-zero-input"))}</label></div><div class="ho-entry"><input id="pbr-zero" value="${esc(pbrZeroText)}" disabled autocomplete="off" /></div><p class="ho-hint">${esc(t("pbr-fixed-tip"))}</p></div>`;
  const rail = `<h2>${esc(t("nav-pbr"))}</h2>
    <div class="checks">
      <label class="ho-check" title="${esc(t("pbr-optimal-tip"))}"><input type="radio" name="pbr-mode" value="optimal" ${pbrMode !== "fixed" ? "checked" : ""} /> ${esc(t("pbr-optimal"))}</label>
      <label class="ho-check" title="${esc(t("pbr-fixed-tip"))}"><input type="radio" name="pbr-mode" value="fixed" ${pbrMode === "fixed" ? "checked" : ""} /> ${esc(t("pbr-fixed"))}</label>
    </div>
    ${hoSelect("pbr-preset", t("pbr-preset"), `<option value="moose" ${pbrZonePreset === "moose" ? "selected" : ""}>${esc(t("pbr-moose"))}</option><option value="roe" ${pbrZonePreset === "roe" ? "selected" : ""}>${esc(t("pbr-roe"))}</option><option value="custom" ${pbrZonePreset === "custom" ? "selected" : ""}>${esc(t("impact-custom"))}</option>`)}
    ${qtyField("zone", t("pbr-zone"), pbrZoneText(), pbrZoneUnit(), "length")}
    ${zero}
    <button type="button" class="ho-btn primary" id="pbr-build">${esc(t("apply"))}</button>`;
  const main = `<div id="pbr-body">${pbrResult ? pbrView(pbrResult) : `<p class="ho-hint">${esc(t(pbrMode === "fixed" ? "pbr-fixed-note" : "pbr-note"))}</p>`}</div>`;
  return frame({ rail, main, railLabel: t("nav-pbr") });
}

function pbrZoneText() {
  if (pbrResult && pbrResult.zone_text) return pbrResult.zone_text;
  if (pbrZonePreset === "roe") return "10 cm";
  if (pbrZonePreset === "custom") return "12 cm";
  return "15 cm";
}

function pbrZoneUnit() {
  return pbrZonePreset === "custom" ? units().small : "cm";
}

function pbrView(data) {
  const confirm = data.mode === "fixed" ? "" : (pbrPreview && !pbrPreview.applied
    ? `<p>${esc(t("pbr-apply-preview"))} ${esc(pbrPreview.next_text)}. <button type="button" id="pbr-confirm">${esc(t("pbr-confirm"))}</button></p>`
    : `<button type="button" id="pbr-apply">${esc(t("pbr-apply"))}</button>`);
  const inside = data.starts_inside ? `<p class="hint">${esc(t("pbr-starts-inside"))}</p>` : "";
  const above = data.exits_above ? `<p class="hint">${esc(t("pbr-exits-above"))}</p>` : "";
  const unit = shownUnit(data.length_unit || "");
  const small = shownUnit(data.small_unit || "");
  const kv = (label, value) => `<dt>${esc(label)}</dt><dd>${esc(fixedDisplay(value, 2))}</dd><span class="ho-unit">${esc(unit)}</span>`;
  return `<div class="grid2">
    <section class="ho-card">
      <div id="pbr-chart"></div>
      <div class="ho-seg wrap" id="pbr-spans">${pbrSpanButtons(data)}</div>
    </section>
    <section class="ho-card stack">
      <dl class="ho-kv">
        ${kv(t("pbr-start-label"), data.near_display)}
        ${kv(t("pbr-near-zero-label"), data.near_zero_display)}
        ${kv(t("pbr-far-zero-label"), data.far_zero_display)}
        ${kv(t("pbr-end-label"), data.far_display)}
        <dt>${esc(t("pbr-ordinate"))}</dt><dd>${esc(fixedDisplay(data.ordinate_display, 2))}</dd><span class="ho-unit">${esc(small)}</span>
        <dt>${esc(t("pbr-at"))}</dt><dd>${esc(fixedDisplay(data.ordinate_range_display, 2))}</dd><span class="ho-unit">${esc(unit)}</span>
      </dl>
      ${confirm}
      <ul class="ho-notes">
        <li class="info">${esc(t(data.mode === "fixed" ? "pbr-fixed-note" : "pbr-note"))}</li>
        <li class="info">${esc(t("pbr-band"))}</li>
        ${inside ? `<li class="warn">${esc(t("pbr-starts-inside"))}</li>` : ""}
        ${above ? `<li class="warn">${esc(t("pbr-exits-above"))}</li>` : ""}
      </ul>
      <details>
        <summary>${esc(t("pbr-more"))}</summary>
        <ul class="ho-notes">
          <li class="info">${esc(t("pbr-start-help"))}</li>
          <li class="info">${esc(t("pbr-near-zero-help"))}</li>
          <li class="info">${esc(t("pbr-far-zero-help"))}</li>
          <li class="info">${esc(t("pbr-end-help"))}</li>
          <li class="info">${esc(t("pbr-ordinate-help"))}</li>
          <li class="info">${esc(t("pbr-still-air"))}</li>
          <li class="info">${esc(t("pbr-zoom-hint"))}</li>
        </ul>
      </details>
    </section>
  </div>`;
}

function pbrSpanButtons(data) {
  const unit = (data.x && data.x.unit) || data.length_unit || "m";
  const choices = [
    ["zone", t("pbr-span-zone")],
    ["300", pbrRangeLabel(300, unit)],
    ["800", pbrRangeLabel(800, unit)],
    ["full", t("pbr-span-full")],
  ];
  return choices.map(([id, label]) =>
    `<button type="button" data-pbr-span="${id}" aria-pressed="${pbrSpan === id}">${esc(label)}</button>`).join("");
}

function pbrRangeLabel(metres, unit) {
  const factor = toSi(1, "length", unit) || 1;
  const shown = metres / factor;
  const text = shown >= 100 ? String(Math.round(shown)) : String(Math.round(shown * 10) / 10);
  return `${text} ${unit}`;
}

function pbrExtent(data) {
  const samples = data.samples || [];
  if (!samples.length) {
    const x = data.x || { min: 0, max: 1 };
    return { min: Number(x.min) || 0, max: Number(x.max) || 1 };
  }
  const min = Number(samples[0].range) || 0;
  const max = Number(samples[samples.length - 1].range) || min + 1;
  return { min, max: Math.max(max, min + 1e-6) };
}

function pbrPresetWindow(data) {
  const extent = pbrExtent(data);
  if (pbrSpan === "zone") {
    const unit = (data.x && data.x.unit) || data.length_unit || "m";
    const factor = toSi(1, "length", unit) || 1;
    const near = Number(data.near_m);
    const far = Number(data.far_m);
    if (Number.isFinite(near) && Number.isFinite(far) && far > near) {
      const pad = (far - near) * 0.08;
      const min = Math.max(0, near - pad) / factor;
      const max = (far + pad) / factor;
      return { min: Math.max(extent.min, min), max: Math.min(extent.max, Math.max(min + 1, max)) };
    }
  }
  if (pbrSpan !== "300" && pbrSpan !== "800") return extent;
  const unit = (data.x && data.x.unit) || "m";
  const shown = Number(pbrSpan) / (toSi(1, "length", unit) || 1);
  return { min: extent.min, max: Math.min(extent.max, Math.max(extent.min + 1, shown)) };
}

function clampPbrWindow(min, max, extent) {
  const full = Math.max(extent.max - extent.min, 1e-6);
  let span = max - min;
  const minSpan = Math.max(full * 0.02, 1e-3);
  if (!Number.isFinite(span) || span < minSpan) span = minSpan;
  if (span > full) span = full;
  let start = min;
  if (start < extent.min) start = extent.min;
  if (start + span > extent.max) start = extent.max - span;
  return { min: start, max: start + span };
}

function pbrVisible(data) {
  const extent = pbrExtent(data);
  if (pbrWindow) return clampPbrWindow(pbrWindow.min, pbrWindow.max, extent);
  return clampPbrWindow(pbrPresetWindow(data).min, pbrPresetWindow(data).max, extent);
}

function tickDecimals(step) {
  let scaled = step;
  let decimals = 0;
  while (decimals < 8 && Math.abs(scaled - Math.round(scaled)) > 1e-6) {
    scaled *= 10;
    decimals += 1;
  }
  return decimals;
}

function snapTick(value, step) {
  const scale = 10 ** tickDecimals(step);
  return Math.round(value * scale) / scale;
}

function formatTick(value, step) {
  const snapped = snapTick(value, step);
  if (!Number.isFinite(snapped) || Math.abs(snapped) < Math.max(Math.abs(step), 1) * 1e-9) return "0";
  let text = snapped.toFixed(tickDecimals(step));
  if (text.includes(".")) text = text.replace(/0+$/, "").replace(/\.$/, "");
  return text;
}

function niceStep(span, maxTicks) {
  const raw = Math.abs(span) / Math.min(12, Math.max(2, maxTicks));
  if (!Number.isFinite(raw) || raw === 0) return 1;
  const exp = Math.floor(Math.log10(raw));
  const base = 10 ** exp;
  const frac = raw / base;
  const nice = [1, 2, 2.5, 5, 10].find((step) => step + 1e-9 >= frac) || 10;
  return nice * base;
}

function niceAxis(dataMin, dataMax, maxTicks, includeZero) {
  let lo = Math.min(dataMin, dataMax);
  let hi = Math.max(dataMin, dataMax);
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) {
    lo = 0;
    hi = 1;
  }
  if (includeZero) {
    lo = Math.min(lo, 0);
    hi = Math.max(hi, 0);
  }
  if (hi - lo < 1e-9) {
    lo -= 1;
    hi += 1;
  }
  const step = niceStep(hi - lo, maxTicks);
  const start = snapTick(Math.floor(lo / step) * step, step);
  const end = snapTick(Math.ceil(hi / step) * step, step);
  const ticks = [];
  const labels = [];
  let value = start;
  for (let guard = 0; value <= end + step * 1e-6 && guard < 48; guard += 1) {
    const snapped = snapTick(value, step);
    if (!ticks.length || Math.abs(snapped - ticks[ticks.length - 1]) > step * 1e-6) {
      ticks.push(snapped);
      labels.push(formatTick(snapped, step));
    }
    value += step;
  }
  if (!ticks.length) {
    ticks.push(0);
    labels.push("0");
  }
  return { min: ticks[0], max: ticks[ticks.length - 1], ticks, labels };
}

function paintPbrChart() {
  const host = document.getElementById("pbr-chart");
  const data = pbrResult;
  if (!host || !data || !(data.samples || []).length || !data.x || !data.y) return;
  const samples = data.samples;
  const shown = pbrVisible(data);
  const visible = samples.filter((sample, index) => {
    const range = Number(sample.range);
    const prev = index ? Number(samples[index - 1].range) : range;
    const next = index + 1 < samples.length ? Number(samples[index + 1].range) : range;
    return (range >= shown.min && range <= shown.max)
      || (range < shown.min && next > shown.min)
      || (range > shown.max && prev < shown.max);
  });
  let yLo = Infinity;
  let yHi = -Infinity;
  visible.forEach((sample) => {
    const path = Number(sample.path);
    if (Number.isFinite(path)) {
      yLo = Math.min(yLo, path);
      yHi = Math.max(yHi, path);
    }
  });
  const radius = Number(data.radius_display) || 0;
  if (!Number.isFinite(yLo) || !Number.isFinite(yHi)) {
    yLo = -radius;
    yHi = radius;
  }
  yLo = Math.min(yLo, -radius, 0);
  yHi = Math.max(yHi, radius, 0);
  const xAxis = niceAxis(shown.min, shown.max, 6, shown.min <= 0 && shown.max >= 0);
  const yAxis = niceAxis(yLo, yHi, 5, true);
  const width = Math.max(640, host.clientWidth || 720);
  const height = Math.round(Math.max(320, width * 0.46));
  const padL = 78;
  const padR = 18;
  const padT = 18;
  const padB = 36;
  const plotW = width - padL - padR;
  const plotH = height - padT - padB;
  const xSpan = (xAxis.max - xAxis.min) || 1;
  const ySpan = (yAxis.max - yAxis.min) || 1;
  const xOf = (value) => padL + (value - xAxis.min) / xSpan * plotW;
  const yOf = (value) => padT + (yAxis.max - value) / ySpan * plotH;
  let marks = `<rect class="frame" x="${padL}" y="${padT}" width="${plotW.toFixed(1)}" height="${plotH.toFixed(1)}" fill="none" />`;
  yAxis.ticks.forEach((tick, index) => {
    const y = yOf(tick);
    marks += `<line class="grid" x1="${padL}" x2="${width - padR}" y1="${y.toFixed(1)}" y2="${y.toFixed(1)}" />`;
    marks += `<text class="tick" fill="var(--text-secondary)" x="${padL - 8}" y="${(y + 4).toFixed(1)}" text-anchor="end">${esc(yAxis.labels[index])}</text>`;
  });
  xAxis.ticks.forEach((tick, index) => {
    const x = xOf(tick);
    marks += `<line class="grid" y1="${padT}" y2="${height - padB}" x1="${x.toFixed(1)}" x2="${x.toFixed(1)}" />`;
    marks += `<text class="tick" fill="var(--text-secondary)" x="${x.toFixed(1)}" y="${height - 10}" text-anchor="middle">${esc(xAxis.labels[index])}</text>`;
  });
  const band = (value) => `<line class="band zone" x1="${padL}" x2="${width - padR}" y1="${yOf(value).toFixed(1)}" y2="${yOf(value).toFixed(1)}" />`;
  const drawn = visible.length ? visible : samples;
  const d = drawn.map((sample, index) => `${index ? "L" : "M"}${xOf(sample.range).toFixed(1)},${yOf(sample.path).toFixed(1)}`).join(" ");
  host.innerHTML = `<p class="legend"><span><i class="swatch"></i>${esc(t("path"))}</span><span>${esc(t("pbr-band"))}</span><span>${esc(t("chart-los"))}</span></p>
    <p class="axis-title">${esc(data.y.unit)}, ${esc(t("chart-above-los"))}</p>
    <svg class="chart" data-pad-l="${padL}" data-pad-r="${padR}" viewBox="0 0 ${width} ${height}" role="img" aria-label="${esc(t("nav-pbr"))}">
      ${marks}
      ${band(radius)}${band(-radius)}
      <line class="zero" x1="${padL}" x2="${width - padR}" y1="${yOf(0).toFixed(1)}" y2="${yOf(0).toFixed(1)}" />
      <text class="zero-label" fill="var(--text-secondary)" x="${(width - padR - 4)}" y="${Math.max(padT + 14, yOf(0) - 6).toFixed(1)}" text-anchor="end">${esc(t("chart-los"))}</text>
      <path d="${d}" fill="none" stroke="var(--chart-0)" stroke-width="2" />
    </svg>
    <p class="axis-title">${esc(t("range"))} (${esc(data.x.unit)})</p>`;
}

function markPbrSpan() {
  document.querySelectorAll("[data-pbr-span]").forEach((button) => {
    button.setAttribute("aria-pressed", button.dataset.pbrSpan === pbrSpan ? "true" : "false");
  });
}

function pbrPlotFraction(event) {
  const host = document.getElementById("pbr-chart");
  const svg = host && host.querySelector("svg");
  if (!svg) return 0.5;
  const rect = svg.getBoundingClientRect();
  const padL = Number(svg.dataset.padL) || 78;
  const padR = Number(svg.dataset.padR) || 18;
  const width = svg.viewBox.baseVal.width || rect.width || 1;
  const plotLeft = rect.left + (padL / width) * rect.width;
  const plotWidth = ((width - padL - padR) / width) * rect.width;
  if (plotWidth <= 1) return 0.5;
  return Math.min(1, Math.max(0, (event.clientX - plotLeft) / plotWidth));
}

function zoomPbr(anchor, factor) {
  if (!pbrResult) return;
  const extent = pbrExtent(pbrResult);
  const current = pbrVisible(pbrResult);
  const span = (current.max - current.min) * factor;
  const at = current.min + (current.max - current.min) * anchor;
  const min = at - span * anchor;
  pbrWindow = clampPbrWindow(min, min + span, extent);
  pbrSpan = "custom";
  markPbrSpan();
  paintPbrChart();
}

function panPbr(min, max) {
  if (!pbrResult) return;
  pbrWindow = clampPbrWindow(min, max, pbrExtent(pbrResult));
  pbrSpan = "custom";
  markPbrSpan();
  paintPbrChart();
}

function bindPbrChart() {
  const host = document.getElementById("pbr-chart");
  if (!host || host.dataset.bound) return;
  host.dataset.bound = "1";
  host.addEventListener("wheel", (event) => {
    if (!pbrResult) return;
    event.preventDefault();
    const factor = event.deltaY > 0 ? 1.18 : 1 / 1.18;
    zoomPbr(pbrPlotFraction(event), factor);
  }, { passive: false });
  let drag = null;
  host.addEventListener("pointerdown", (event) => {
    if (!pbrResult || event.button !== 0) return;
    const current = pbrVisible(pbrResult);
    drag = { x: event.clientX, min: current.min, max: current.max, width: host.getBoundingClientRect().width };
    host.setPointerCapture(event.pointerId);
  });
  host.addEventListener("pointermove", (event) => {
    if (!drag) return;
    const span = drag.max - drag.min;
    const dx = (event.clientX - drag.x) / Math.max(1, drag.width) * span;
    const extent = pbrExtent(pbrResult);
    pbrWindow = clampPbrWindow(drag.min - dx, drag.max - dx, extent);
    pbrSpan = "custom";
    markPbrSpan();
    paintPbrChart();
  });
  const end = () => { drag = null; };
  host.addEventListener("pointerup", end);
  host.addEventListener("pointercancel", end);
}

function bindPbrSpans() {
  document.querySelectorAll("[data-pbr-span]").forEach((button) => {
    button.addEventListener("click", () => {
      pbrSpan = button.dataset.pbrSpan;
      pbrWindow = null;
      const host = document.getElementById("pbr-body");
      if (host && pbrResult) {
        host.innerHTML = pbrView(pbrResult);
        bindPbrActions();
      }
    });
  });
}

function bindPbr() {
  const preset = document.getElementById("pbr-preset");
  if (preset) preset.addEventListener("change", () => {
    pbrZonePreset = preset.value;
    pbrResult = null;
    pbrPreview = null;
    render();
  });
  document.querySelectorAll("input[name=pbr-mode]").forEach((input) => input.addEventListener("change", () => {
    pbrMode = input.value;
    pbrPreview = null;
    render();
  }));
  const zero = document.getElementById("pbr-zero");
  if (zero) zero.addEventListener("input", () => { pbrZeroText = zero.value; });
  const build = document.getElementById("pbr-build");
  if (build) build.addEventListener("click", runPbr);
  bindPbrActions();
  if (!pbrResult && boot.settings.active_profile_id) runPbr();
  else if (!boot.settings.active_profile_id) {
    const host = document.getElementById("pbr-body");
    if (host && !pbrResult) host.innerHTML = emptyCta("pbr-note");
  }
}

function bindPbrActions() {
  const apply = document.getElementById("pbr-apply");
  if (apply) apply.addEventListener("click", () => runApplyZero(false));
  const confirm = document.getElementById("pbr-confirm");
  if (confirm) confirm.addEventListener("click", () => runApplyZero(true));
  paintPbrChart();
  bindPbrSpans();
  bindPbrChart();
  requestAnimationFrame(() => paintPbrChart());
}

async function runPbr() {
  const host = document.getElementById("pbr-body");
  try {
    const body = solveBody();
    body.zone = qtyPayload("zone");
    body.mode = pbrMode;
    if (pbrMode === "fixed") {
      const zero = document.getElementById("pbr-zero");
      if (zero) pbrZeroText = zero.value;
      body.zero = qtyPayload("pbr-zero") || { text: pbrZeroText, unit: units().length };
    }
    pbrResult = await call("point_blank", { body });
    pbrResult.zone_text = body.zone.text;
    pbrPreview = null;
    if (host) host.innerHTML = pbrView(pbrResult);
    bindPbrActions();
  } catch (error) {
    if (host) host.innerHTML = `<p class="error">${esc(error)}</p>`;
  }
}

async function runApplyZero(confirm) {
  const host = document.getElementById("pbr-body");
  try {
    const body = solveBody();
    body.zone = qtyPayload("zone") || { text: pbrZoneText(), unit: pbrZoneUnit() };
    body.confirm = confirm;
    pbrPreview = await call("apply_zero", { body });
    if (confirm) {
      boot = await call("bootstrap");
      pbrPreview = null;
      render();
      return;
    }
    if (host && pbrResult) {
      host.innerHTML = pbrView(pbrResult);
      bindPbrActions();
    }
  } catch (error) {
    if (host) host.innerHTML = `<p class="error">${esc(error)}</p>`;
  }
}

function truingHtml() {
  const profile = activeProfile();
  const history = (profile && profile.truing && profile.truing.history) || [];
  const active = (profile && profile.truing && profile.truing.active && profile.truing.active.label) || t("truing-none");
  const rows = history.map((entry) => `<div class="stack">
    <span>${esc(entry.summary || "")}</span>
    <div class="btnrow">
      <button type="button" class="ho-btn" data-history="${esc(entry.id)}" data-action="activate">${esc(t("truing-activate"))}</button>
      <button type="button" class="ho-btn ghost" data-history="${esc(entry.id)}" data-action="revert">${esc(t("truing-revert"))}</button>
    </div>
  </div>`).join("");
  const rail = `<h2>${esc(t("nav-truing"))}</h2>
    <p class="ho-hint">${esc(t("truing-intro"))}</p>
    ${hoSelect("truing-kind", t("truing-kind"), `<option value="mv" ${truingKind === "mv" ? "selected" : ""}>${esc(t("truing-mv"))}</option><option value="drag" ${truingKind === "drag" ? "selected" : ""}>${esc(t("truing-drag"))}</option><option value="tracking" ${truingKind === "tracking" ? "selected" : ""}>${esc(t("truing-tracking"))}</option>`, t("truing-kind-tip"))}
    <div id="truing-fields">${truingFields()}</div>
    <button type="button" class="ho-btn primary" id="truing-preview">${esc(t("settings-preview"))}</button>`;
  const main = `<div class="grid2">
      <section class="ho-card" id="truing-result">${truingPreview ? truingDiff(truingPreview) : `<p class="ho-hint">${esc(t("truing-intro"))}</p>`}</section>
      <section class="ho-card stack">
        <header><h2>${esc(t("truing-history"))}</h2><span class="meta">${esc(t("truing-active"))}: ${esc(active)}</span></header>
        <div class="ho-list">${rows || `<p class="ho-hint">${esc(t("truing-history-empty"))}</p>`}</div>
        <p class="ho-hint">${esc(t("truing-confirm-note"))}</p>
      </section>
    </div>`;
  return frame({ rail, main, railLabel: t("nav-truing") });
}

function truingFields() {
  if (truingKind === "drag") {
    return `<label class="field wide" title="${esc(t("truing-lines-tip"))}">${esc(t("truing-lines"))}<textarea id="truing-lines" rows="5" title="${esc(t("truing-lines-tip"))}">${esc(truingForm.lines)}</textarea></label>
      <label title="${esc(t("truing-piecewise-tip"))}"><input type="checkbox" id="truing-piecewise" ${truingForm.piecewise ? "checked" : ""} /> ${esc(t("truing-piecewise"))}</label>
      <p class="hint">${esc(t("truing-drag-guidance"))}</p>`;
  }
  if (truingKind === "tracking") {
    return `<div class="cluster">
        <span title="${esc(t("truing-advertised-tip"))}">${qtyField("advertised", t("truing-advertised"), truingForm.advertised, "mrad", "angle")}</span>
        <span title="${esc(t("truing-observed-tip"))}">${qtyField("observed", t("truing-observed"), truingForm.observed, "mrad", "angle")}</span>
      </div>
      <p class="hint">${esc(t("truing-tracking-guidance"))}</p>`;
  }
  return `<div class="cluster">
      <span title="${esc(t("truing-range-tip"))}">${qtyField("observe_range", t("range"), truingForm.range, units().length, "length", null, null, "observe_range")}</span>
      <span title="${esc(t("truing-elev-tip"))}">${qtyField("observe_elevation", t("elevation"), truingForm.elevation, "mrad", "angle")}</span>
    </div>
    <div class="cluster">
      <label class="field" title="${esc(t("truing-shots-tip"))}">${esc(t("truing-shots"))}<input id="truing-shots" value="${esc(truingForm.shots)}" title="${esc(t("truing-shots-tip"))}" /></label>
      <label class="field" title="${esc(t("truing-group-tip"))}">${esc(t("truing-group"))}<input id="truing-group" value="${esc(truingForm.group)}" title="${esc(t("truing-group-tip"))}" /></label>
    </div>
    <p class="hint">${esc(t("truing-mv-guidance"))}</p>`;
}

function snapshotBits(snap) {
  if (!snap || typeof snap !== "object") return t("truing-none");
  if (snap.label) return String(snap.label);
  const bits = [];
  if (snap.mv_mps != null && snap.mv_mps !== "") bits.push(`MV ${Number(snap.mv_mps).toFixed(1)} m/s`);
  if (snap.tracking_factor != null && snap.tracking_factor !== "") bits.push(Number(snap.tracking_factor).toFixed(3));
  return bits.join(" · ") || t("truing-none");
}

function truingDiff(preview) {
  const detail = preview.detail || {};
  const warnings = [];
  if (detail.large_change) warnings.push(t("truing-large"));
  if (detail.before_transonic === false && preview.kind === "mv") warnings.push(t("truing-transonic"));
  if (detail.guidance) warnings.push(detail.guidance);
  const apply = preview.applied ? "" : `<button type="button" class="ho-btn primary" id="truing-apply">${esc(t("truing-apply"))}</button>`;
  const changed = preview.kind === "drag"
    ? t("truing-result-drag")
    : preview.kind === "tracking"
      ? t("truing-result-tracking")
      : t("truing-result-mv");
  const notes = warnings.filter(Boolean).map((item) => `<li class="warn">${esc(item)}</li>`).join("");
  return `<p class="big">${esc(preview.summary || preview.action || "")}</p>
    <p class="ho-hint">${esc(changed)}</p>
    <dl class="ho-kv">
      <dt>${esc(t("truing-before"))}</dt><dd>${esc(snapshotBits(preview.before))}</dd><span class="ho-unit"></span>
      <dt>${esc(t("truing-after"))}</dt><dd>${esc(snapshotBits(preview.after))}</dd><span class="ho-unit"></span>
    </dl>
    ${notes ? `<ul class="ho-notes">${notes}</ul>` : ""}
    ${apply}`;
}

function readTruingForm() {
  truingKind = (document.getElementById("truing-kind") || {}).value || truingKind;
  const lines = document.getElementById("truing-lines");
  if (lines) truingForm.lines = lines.value;
  const piecewise = document.getElementById("truing-piecewise");
  if (piecewise) truingForm.piecewise = piecewise.checked;
  const range = document.getElementById("observe_range");
  if (range) truingForm.range = range.value;
  const elevation = document.getElementById("observe_elevation");
  if (elevation) truingForm.elevation = elevation.value;
  const shots = document.getElementById("truing-shots");
  if (shots) truingForm.shots = shots.value;
  const group = document.getElementById("truing-group");
  if (group) truingForm.group = group.value;
  const advertised = document.getElementById("advertised");
  if (advertised) truingForm.advertised = advertised.value;
  const observed = document.getElementById("observed");
  if (observed) truingForm.observed = observed.value;
}

function truingBody(confirm) {
  readTruingForm();
  const body = solveBody();
  body.kind = truingKind;
  body.confirm = confirm;
  body.shots = truingForm.shots;
  body.group_text = truingForm.group;
  if (truingKind === "mv") {
    body.observe_range = qtyPayload("observe_range") || { text: truingForm.range, unit: units().length };
    body.observe_elevation = qtyPayload("observe_elevation") || { text: truingForm.elevation, unit: "mrad" };
  } else if (truingKind === "drag") {
    body.piecewise = truingForm.piecewise;
    body.observations = truingForm.lines.split("\n").map((line) => {
      const parts = line.split(",").map((part) => part.trim()).filter(Boolean);
      if (parts.length < 2) return null;
      return {
        range: { text: parts[0], unit: units().length },
        elevation: { text: parts[1], unit: "mrad" },
        shots: parts[2] || "",
        group_text: parts[3] || "",
      };
    }).filter(Boolean);
  } else {
    body.advertised = qtyPayload("advertised") || { text: truingForm.advertised, unit: "mrad" };
    body.observed = qtyPayload("observed") || { text: truingForm.observed, unit: "mrad" };
  }
  return body;
}

function bindTruing() {
  const kind = document.getElementById("truing-kind");
  if (kind) kind.addEventListener("change", () => {
    truingKind = kind.value;
    truingPreview = null;
    const host = document.getElementById("truing-fields");
    if (host) {
      host.innerHTML = truingFields();
      wireSteppers(host);
    }
  });
  const preview = document.getElementById("truing-preview");
  if (preview) preview.addEventListener("click", () => runTruing(false));
  const apply = document.getElementById("truing-apply");
  if (apply) apply.addEventListener("click", () => runTruing(true));
  document.querySelectorAll("[data-history]").forEach((button) => button.addEventListener("click", () => runTruingHistory(button.dataset.action, button.dataset.history)));
}

async function runTruing(confirm) {
  const host = document.getElementById("truing-result");
  try {
    truingPreview = await call("truing", { body: truingBody(confirm) });
    if (confirm) {
      boot = await call("bootstrap");
      render();
      return;
    }
    if (host) {
      host.innerHTML = truingDiff(truingPreview);
      const apply = document.getElementById("truing-apply");
      if (apply) apply.addEventListener("click", () => runTruing(true));
    }
  } catch (error) {
    if (host) host.innerHTML = `<p class="error">${esc(error)}</p>`;
  }
}

async function runTruingHistory(action, historyId) {
  const host = document.getElementById("truing-result");
  try {
    const profile = activeProfile();
    const preview = await call("truing", { body: { profile_id: profile && profile.id, action, history_id: historyId, confirm: false } });
    const accepted = window.confirm(`${preview.summary || action}`);
    if (!accepted) {
      if (host) host.innerHTML = truingDiff(preview);
      return;
    }
    await call("truing", { body: { profile_id: profile.id, action, history_id: historyId, confirm: true } });
    boot = await call("bootstrap");
    render();
  } catch (error) {
    if (host) host.innerHTML = `<p class="error">${esc(error)}</p>`;
  }
}

function chronoHtml() {
  const loads = (boot.equipment.loads || []).map((item) => `<option value="${esc(item.id)}" ${item.id === chronoForm.loadId ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const stats = chronoResult ? `<p>${esc(t("chrono-count"))} ${esc(chronoResult.count)} · ${esc(t("chrono-mean"))} ${esc(chronoResult.mean_display)} ${esc(chronoResult.unit)}</p>
    <p>${esc(t("chrono-sd"))} ${esc(chronoResult.sd_display)} · ${esc(t("chrono-es"))} ${esc(chronoResult.es_display)} · ${esc(chronoResult.min_display)}–${esc(chronoResult.max_display)}</p>
    <p>${esc(t("chrono-outliers"))} ${esc((chronoResult.outliers || []).join(", ") || "—")}</p>
    <p class="hint">${esc(chronoResult.outlier_note || t("chrono-keep"))}</p>` : "";
  return `<div class="fields">
    <label class="field">${esc(t("chrono-shots"))}<textarea id="chrono-shots" rows="5">${esc(chronoForm.shots)}</textarea></label>
    <p class="hint">${esc(t("chrono-paste"))}</p>
    ${qtyField("chrono-temperature", t("temperature"), chronoForm.temperature, units().temperature, "temperature")}
    <label class="field">${esc(t("label"))}<input id="chrono-label" value="${esc(chronoForm.label)}" /></label>
    <label class="field">${esc(t("load-link"))}<select id="chrono-load">${loads}</select></label>
    <div class="row">
      <button type="button" id="chrono-stats">${esc(t("chrono-stats"))}</button>
      <button type="button" id="chrono-save">${esc(t("save"))}</button>
      <button type="button" id="chrono-mv">${esc(t("chrono-set-mv"))}</button>
      <button type="button" id="chrono-fit">${esc(t("chrono-fit"))}</button>
      <button type="button" id="chrono-apply">${esc(t("chrono-apply-powder"))}</button>
    </div>
    <div id="chrono-result">${stats}</div>
    <p id="chrono-msg" class="meta"></p>
  </div>`;
}

function readChronoForm() {
  const shots = document.getElementById("chrono-shots");
  if (shots) chronoForm.shots = shots.value;
  const temperature = document.getElementById("chrono-temperature");
  if (temperature) chronoForm.temperature = temperature.value;
  const label = document.getElementById("chrono-label");
  if (label) chronoForm.label = label.value;
  const load = document.getElementById("chrono-load");
  if (load) chronoForm.loadId = load.value;
}

function bindChrono() {
  ["chrono-stats", "chrono-save", "chrono-mv", "chrono-fit", "chrono-apply"].forEach((id) => {
    const node = document.getElementById(id);
    if (node) node.addEventListener("click", () => runChrono(id));
  });
}

async function runChrono(id) {
  const msg = document.getElementById("chrono-msg");
  const host = document.getElementById("chrono-result");
  readChronoForm();
  try {
    if (id === "chrono-stats" || id === "chrono-save") {
      const save = id === "chrono-save";
      const confirm = save ? window.confirm(t("chrono-save-confirm")) : false;
      if (save && !confirm) return;
      chronoResult = await call("chrono_session", { body: {
        op: save ? "save" : "stats",
        shots: chronoForm.shots,
        temperature: { text: chronoForm.temperature, unit: units().temperature },
        label: chronoForm.label,
        load_id: chronoForm.loadId,
        confirm,
      } });
      if (save) boot = await call("bootstrap");
      render();
      return;
    }
    if (id === "chrono-mv") {
      if (!chronoResult) return;
      if (!window.confirm(t("chrono-mv-confirm"))) return;
      await call("chrono_session", { body: {
        op: "set_mv",
        load_id: chronoForm.loadId,
        mean_mps: chronoResult.mean_mps,
        temperature_c: chronoResult.temperature_c,
        confirm: true,
      } });
      boot = await call("bootstrap");
      if (msg) msg.textContent = t("saved");
      render();
      return;
    }
    const apply = id === "chrono-apply";
    if (apply && !window.confirm(t("chrono-powder-confirm"))) return;
    const fit = await call("chrono_session", { body: {
      op: apply ? "apply_powder" : "fit",
      load_id: chronoForm.loadId,
      confirm: apply,
    } });
    if (apply) boot = await call("bootstrap");
    if (msg) msg.textContent = `${t("chrono-slope")} ${Number(fit.slope_mps_per_c).toFixed(3)} m/s/°C · R² ${Number(fit.r_squared).toFixed(3)} · n ${fit.n}`;
  } catch (error) {
    if (msg) msg.textContent = String(error);
  }
}

function recoilDefaults() {
  if (recoilForm) return recoilForm;
  const profile = activeProfile();
  const rifle = profile && (boot.equipment.rifles || []).find((item) => item.id === profile.rifle_id);
  const load = profile && (boot.equipment.loads || []).find((item) => item.id === profile.load_id);
  const bullet = load && (boot.equipment.bullets || []).find((item) => item.id === load.bullet_id);
  recoilForm = {
    rifle: (rifle && rifle.weight_text) || "",
    bullet: (bullet && bullet.weight_text) || "",
    charge: "",
    mv: (load && load.mv_text) || "",
    ejecta: `${Math.round((boot.ejecta_mps || 1219.2) * 10) / 10} mps`,
    reduction: "0",
  };
  return recoilForm;
}

function recoilHtml() {
  const form = recoilDefaults();
  const result = recoilResult ? `<p>${esc(t("recoil-velocity"))} ${esc(recoilResult.velocity_display)} ${esc(shownUnit(recoilResult.velocity_unit))} · ${esc(t("energy"))} ${esc(recoilResult.energy_display)} ${esc(recoilResult.energy_unit)}</p>
    <p>${esc(recoilResult.convention || boot.ejecta_convention || "")}</p>
    ${recoilResult.reduction_is_estimate ? `<p class="hint">${esc(t("recoil-estimate"))}</p>` : ""}` : `<p>${esc(boot.ejecta_convention || "")}</p>`;
  return `<div class="fields">
    ${qtyField("recoil-rifle", t("recoil-rifle"), form.rifle, units().mass, "mass")}
    ${qtyField("recoil-bullet", t("recoil-bullet"), form.bullet, units().mass, "mass")}
    ${qtyField("recoil-charge", t("recoil-charge"), form.charge, units().mass, "mass")}
    <p class="hint">${esc(t("recoil-charge-note"))}</p>
    ${qtyField("recoil-mv", t("mv"), form.mv, units().velocity, "velocity")}
    ${qtyField("recoil-ejecta", t("recoil-ejecta"), form.ejecta, "mps", "velocity")}
    <label class="field">${esc(t("recoil-reduction"))}<input id="recoil-reduction" value="${esc(form.reduction)}" inputmode="decimal" /></label>
    <p class="hint">${esc(t("recoil-estimate"))}</p>
    <button type="button" id="recoil-calc">${esc(t("apply"))}</button>
    <div id="recoil-result">${result}</div>
    <p id="recoil-msg" class="error"></p>
  </div>`;
}

function bindRecoil() {
  const button = document.getElementById("recoil-calc");
  if (button) button.addEventListener("click", runRecoil);
}

async function runRecoil() {
  const msg = document.getElementById("recoil-msg");
  const form = recoilDefaults();
  form.rifle = (document.getElementById("recoil-rifle") || {}).value || form.rifle;
  form.bullet = (document.getElementById("recoil-bullet") || {}).value || form.bullet;
  form.charge = (document.getElementById("recoil-charge") || {}).value || "";
  form.mv = (document.getElementById("recoil-mv") || {}).value || form.mv;
  form.ejecta = (document.getElementById("recoil-ejecta") || {}).value || form.ejecta;
  form.reduction = (document.getElementById("recoil-reduction") || {}).value || "0";
  try {
    recoilResult = await call("recoil_calc", { body: {
      rifle: qtyPayload("recoil-rifle"),
      bullet: qtyPayload("recoil-bullet"),
      charge: qtyPayload("recoil-charge"),
      mv: qtyPayload("recoil-mv"),
      ejecta: qtyPayload("recoil-ejecta"),
      reduction: Number(String(form.reduction).replace(",", ".")) || 0,
    } });
    const host = document.getElementById("recoil-result");
    if (host) host.innerHTML = recoilHtml().includes("unused") ? "" : `<p>${esc(t("recoil-velocity"))} ${esc(recoilResult.velocity_display)} ${esc(shownUnit(recoilResult.velocity_unit))} · ${esc(t("energy"))} ${esc(recoilResult.energy_display)} ${esc(recoilResult.energy_unit)}</p><p>${esc(recoilResult.convention || "")}</p>${recoilResult.reduction_is_estimate ? `<p class="hint">${esc(t("recoil-estimate"))}</p>` : ""}`;
  } catch (error) {
    if (msg) msg.textContent = String(error);
  }
}

function targetOptions(selected, families) {
  const library = (boot.targets && boot.targets.library) || [];
  const saved = (boot.equipment && boot.equipment.targets) || [];
  const seen = new Set();
  return library.concat(saved).filter((item) => {
    if (!item || seen.has(item.id)) return false;
    if (families && !families.includes(item.family)) return false;
    seen.add(item.id);
    return true;
  }).map((item) => `<option value="${esc(item.id)}" ${item.id === selected ? "selected" : ""}>${esc(item.name)} — ${esc(item.subtitle || "")}</option>`).join("");
}

function libraryTarget(id) {
  const saved = ((boot.equipment && boot.equipment.targets) || []).find((item) => item.id === id);
  if (saved) return saved;
  return ((boot.targets && boot.targets.library) || []).find((item) => item.id === id) || null;
}

function pathStyle(path) {
  if (!path.placeholder) return { dash: "", stroke: "var(--text)" };
  if (path.role === "silhouette") return { dash: 'stroke-dasharray="2 7"', stroke: "var(--text-secondary)" };
  if (path.role === "hit_area") return { dash: 'stroke-dasharray="10 4"', stroke: "var(--warning)" };
  return { dash: 'stroke-dasharray="4 3"', stroke: "var(--accent-text)" };
}

function targetLegend(spec) {
  const roles = new Set((spec.paths || []).filter((path) => path.placeholder).map((path) => path.role));
  const items = [];
  if (roles.has("hit_area")) items.push(`<span><i class="swatch dash-hit"></i>${esc(t("legend-hit"))}</span>`);
  if (roles.has("silhouette")) items.push(`<span><i class="swatch dash-sil"></i>${esc(t("legend-silhouette"))}</span>`);
  if ([...roles].some((role) => role !== "hit_area" && role !== "silhouette")) items.push(`<span><i class="swatch dash-other"></i>${esc(t("legend-outline"))}</span>`);
  if (!items.length) return "";
  return `<p class="legend">${items.join("")}</p>`;
}

function flagHtml(flags) {
  return (flags || []).map((flag) => {
    const kind = t("badge-" + (flag.kind || "placeholder"));
    const detail = t(flag.detail || ("flag-" + flag.code));
    const cls = flag.kind === "verified" || flag.kind === "ok" ? "ok" : "warn";
    return `<p><span class="ho-badge ${cls}">${esc(kind)} · ${esc(flag.code || "")}</span> ${esc(detail)}</p>`;
  }).join("");
}

function lengthLabelFromCm(cm) {
  const unit = units().small || "cm";
  return `${formatLocal(Number(cm) / 100, "length", unit)} ${unit}`;
}

function toCm(value, unit) {
  const metres = toSi(Number(value), "length", unit || "cm");
  return Number.isFinite(metres) ? metres * 100 : NaN;
}

function qtyToCm(id) {
  const payload = qtyPayload(id);
  if (!payload) return null;
  const parsed = parseLocal(payload.text, "length", payload.unit);
  if (!parsed || !Number.isFinite(parsed.si)) return null;
  return parsed.si * 100;
}

function faceSvg(spec, opts) {
  const options = opts || {};
  const showSilhouette = options.showSilhouette !== false;
  const rings = (spec.rings || []).filter((ring) => Number(ring.diameter_cm) > 0);
  const paths = (spec.paths || []).filter((path) => showSilhouette || path.role !== "silhouette");
  const pois = options.pois || [];
  let half = 8;
  rings.forEach((ring) => { half = Math.max(half, Number(ring.diameter_cm) / 2); });
  paths.forEach((path) => (path.points || []).forEach((point) => {
    half = Math.max(half, Math.abs(Number(point[0]) || 0), Math.abs(Number(point[1]) || 0));
  }));
  if (spec.width_cm) half = Math.max(half, Number(spec.width_cm) / 2, Number(spec.height_cm) / 2);
  pois.forEach((poi) => {
    half = Math.max(half, Math.abs(Number(poi.path_cm) || 0) + 4, Math.abs(Number(poi.windage_cm) || 0) + 4);
    half = Math.max(half, Math.abs(Number(poi.wind_left_cm) || 0) + 4, Math.abs(Number(poi.wind_right_cm) || 0) + 4);
  });
  if (options.markCm) half = Math.max(half, Math.abs(options.markCm.x) + 2, Math.abs(options.markCm.y) + 2);
  (options.measured || []).forEach((mark) => {
    half = Math.max(half, Math.abs(Number(mark.x_cm) || 0) + 2, Math.abs(Number(mark.y_cm) || 0) + 2);
  });
  if (!Number.isFinite(half) || half > 800) half = 800;
  const size = 1100;
  const pad = 80;
  const scale = (size - pad * 2) / (half * 2);
  const cx = size / 2;
  const cy = size / 2;
  const px = (x, y) => [cx + x * scale, cy - y * scale];
  const ink = "var(--text)";
  const axis = "var(--text-secondary)";
  let body = "";
  if (spec.cell_cm && spec.width_cm && spec.height_cm) {
    const cell = Number(spec.cell_cm);
    const w = Number(spec.width_cm) / 2;
    const h = Number(spec.height_cm) / 2;
    for (let x = -w; x <= w + 1e-6; x += cell) {
      const [x1, y1] = px(x, -h);
      const [x2, y2] = px(x, h);
      body += `<line stroke="var(--border)" stroke-width="1" x1="${x1.toFixed(1)}" y1="${y1.toFixed(1)}" x2="${x2.toFixed(1)}" y2="${y2.toFixed(1)}" />`;
    }
    for (let y = -h; y <= h + 1e-6; y += cell) {
      const [x1, y1] = px(-w, y);
      const [x2, y2] = px(w, y);
      body += `<line stroke="var(--border)" stroke-width="1" x1="${x1.toFixed(1)}" y1="${y1.toFixed(1)}" x2="${x2.toFixed(1)}" y2="${y2.toFixed(1)}" />`;
    }
  }
  paths.forEach((path) => {
    const points = path.points || [];
    if (!points.length) return;
    const style = pathStyle(path);
    const dashed = style.dash;
    const stroke = style.stroke;
    if (path.closed === false) {
      let d = "";
      for (let i = 0; i + 1 < points.length; i += 2) {
        const a = px(Number(points[i][0]), Number(points[i][1]));
        const b = px(Number(points[i + 1][0]), Number(points[i + 1][1]));
        d += `M ${a[0].toFixed(1)} ${a[1].toFixed(1)} L ${b[0].toFixed(1)} ${b[1].toFixed(1)} `;
      }
      body += `<path d="${d}" fill="none" stroke="${stroke}" stroke-width="2" ${dashed} />`;
      return;
    }
    const d = points.map((point, index) => {
      const at = px(Number(point[0]), Number(point[1]));
      return `${index === 0 ? "M" : "L"} ${at[0].toFixed(1)} ${at[1].toFixed(1)}`;
    }).join(" ") + " Z";
    body += `<path d="${d}" fill="none" stroke="${stroke}" stroke-width="2.5" ${dashed} />`;
  });
  rings.forEach((ring) => {
    const radius = Number(ring.diameter_cm) / 2 * scale;
    const alt = options.ringing === "alt" && ring.label_alt;
    const label = alt || ring.label_primary || lengthLabelFromCm(ring.diameter_cm);
    const stroke = ring.verified === false ? axis : ink;
    const dash = ring.verified === false ? 'stroke-dasharray="6 4"' : "";
    const [lx, ly] = ringLabelPoint(px, Number(ring.diameter_cm) / 2, options.ringing);
    body += `<circle cx="${cx.toFixed(1)}" cy="${cy.toFixed(1)}" r="${radius.toFixed(1)}" fill="none" stroke="${stroke}" stroke-width="2" ${dash} />
      <text fill="${ink}" x="${(lx + 8).toFixed(1)}" y="${(ly - 6).toFixed(1)}">${esc(label)}</text>`;
  });
  if (!rings.length && !paths.length) {
    body += `<text fill="${ink}" x="${cx.toFixed(1)}" y="${cy.toFixed(1)}" text-anchor="middle">${esc(t("target-no-size"))}</text>`;
  }
  pois.forEach((poi, index) => {
    const color = `var(--chart-${index % 6})`;
    const [x, y] = px(Number(poi.windage_cm) || 0, Number(poi.path_cm) || 0);
    const low = px(Number(poi.windage_cm) || 0, Number(poi.mv_low_cm));
    const high = px(Number(poi.windage_cm) || 0, Number(poi.mv_high_cm));
    const left = px(Number(poi.wind_left_cm) || 0, Number(poi.path_cm) || 0);
    const right = px(Number(poi.wind_right_cm) || 0, Number(poi.path_cm) || 0);
    body += `<line stroke="${color}" stroke-width="2" x1="${x.toFixed(1)}" x2="${x.toFixed(1)}" y1="${low[1].toFixed(1)}" y2="${high[1].toFixed(1)}" />
      <line stroke="${color}" stroke-width="2" x1="${left[0].toFixed(1)}" x2="${right[0].toFixed(1)}" y1="${y.toFixed(1)}" y2="${y.toFixed(1)}" />
      <circle cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="7" fill="${color}" />`;
    (poi.brackets || []).forEach((item) => {
      const at = px(Number(item.windage_cm) || 0, Number(poi.path_cm) || 0);
      body += `<circle cx="${at[0].toFixed(1)}" cy="${at[1].toFixed(1)}" r="10" fill="none" stroke="${color}" stroke-width="2" />
        <text fill="${ink}" x="${(at[0] + 12).toFixed(1)}" y="${(at[1] - 8).toFixed(1)}">${esc(item.label || "")}</text>`;
    });
  });
  if (options.markCm) {
    const at = px(options.markCm.x, options.markCm.y);
    body += `<circle cx="${at[0].toFixed(1)}" cy="${at[1].toFixed(1)}" r="8" fill="var(--accent)" />
      <text fill="${ink}" x="${(at[0] + 12).toFixed(1)}" y="${(at[1] - 10).toFixed(1)}">${esc(t("lead"))}</text>`;
  }
  (options.measured || []).forEach((mark) => {
    const at = px(Number(mark.x_cm) || 0, Number(mark.y_cm) || 0);
    const count = Number(mark.count) > 1 ? String(mark.count) : "";
    body += `<circle cx="${at[0].toFixed(1)}" cy="${at[1].toFixed(1)}" r="6" fill="none" stroke="var(--warning)" stroke-width="2" />
      <text fill="${ink}" x="${(at[0] + 8).toFixed(1)}" y="${(at[1] - 8).toFixed(1)}">${esc(count)}</text>`;
  });
  const barCm = half > 30 ? 10 : 5;
  const bar0 = px(-barCm / 2, 0);
  const bar1 = px(barCm / 2, 0);
  const step = half > 80 ? 20 : half > 40 ? 10 : 5;
  const axisY = size - pad + 6;
  let ticks = "";
  for (let cm = Math.ceil(-half / step) * step; cm <= half; cm += step) {
    const [x] = px(cm, 0);
    ticks += `<line stroke="${axis}" x1="${x.toFixed(1)}" x2="${x.toFixed(1)}" y1="${(axisY - 8).toFixed(1)}" y2="${axisY.toFixed(1)}" />
      <text fill="${ink}" x="${x.toFixed(1)}" y="${(axisY + 16).toFixed(1)}" text-anchor="middle">${esc(lengthLabelFromCm(cm))}</text>`;
  }
  return `<svg class="target-face" viewBox="0 0 ${size} ${size}" role="img" aria-label="${esc(options.label || spec.name || t("nav-targets"))}">
    <rect width="${size}" height="${size}" fill="var(--background)" />
    <line stroke="${axis}" x1="${pad}" x2="${size - pad}" y1="${cy}" y2="${cy}" />
    <line stroke="${axis}" x1="${cx}" x2="${cx}" y1="${pad}" y2="${size - pad}" />
    ${ticks}${body}
    <line stroke="${ink}" stroke-width="3" x1="${bar0[0].toFixed(1)}" x2="${bar1[0].toFixed(1)}" y1="${(size - 36).toFixed(1)}" y2="${(size - 36).toFixed(1)}" />
    <text fill="${ink}" x="${cx}" y="${(size - 16).toFixed(1)}" text-anchor="middle">${esc(lengthLabelFromCm(barCm))}</text>
  </svg>`;
}

function ringLabelPoint(px, radiusCm, ringing) {
  if (ringing === "alt") return px(radiusCm, 0);
  const diagonal = radiusCm * Math.SQRT1_2;
  return px(diagonal, diagonal);
}

function paintLeadFace() {
  const host = document.getElementById("lead-face");
  if (!host) return;
  if (!leadTarget) {
    host.innerHTML = "";
    return;
  }
  const solved = lastSolve && lastSolve.lead && lastSolve.lead.target;
  let spec = (solved && solved.id) ? solved : libraryTarget(leadTarget);
  if (!spec) {
    host.innerHTML = "";
    return;
  }
  if (!(solved && solved.id) && leadFacing === "right") {
    spec = JSON.parse(JSON.stringify(spec));
    (spec.paths || []).forEach((path) => (path.points || []).forEach((point) => { point[0] = -point[0]; }));
  }
  const lead = lastSolve && lastSolve.lead;
  const cm = lead ? Number(lead.cm) : NaN;
  host.innerHTML = flagHtml(spec.flags) + faceSvg(spec, {
    showSilhouette: true,
    ringing: "primary",
    markCm: Number.isFinite(cm) ? { x: cm, y: 0 } : null,
    label: spec.name,
  });
}

function reticleChoices() {
  const saved = (boot.equipment && boot.equipment.reticles) || [];
  const rows = (boot.reticles || []).concat(saved);
  return rows.map((item) => `<option value="${esc(item.id)}" ${item.id === reticleState.id ? "selected" : ""}>${esc(item.name)}</option>`).join("");
}

function reticleHtml() {
  const tools = ["line", "hash", "dot", "circle", "text"].map((tool) => `<option value="${tool}" ${reticleState.tool === tool ? "selected" : ""}>${esc(t("tool-" + tool))}</option>`).join("");
  const rail = `<h2>${esc(t("reticle-title"))}</h2>
    <p class="ho-hint">${esc(t("reticle-intro"))}</p>
    ${hoSelect("reticle-pick", t("reticle-pick"), reticleChoices(), t("reticle-pick-tip"))}
    ${hoQty("reticle-mag", t("reticle-mag"), "", "count", "reticle-mag", `<input id="reticle-mag" value="${esc(reticleState.mag)}" inputmode="decimal" title="${esc(t("reticle-mag-tip"))}" autocomplete="off" spellcheck="false" />`)}
    <div class="btnrow">${[5, 10, 20].map((mag) => `<button type="button" class="ho-btn ghost" data-mag="${mag}" title="${esc(t("reticle-mag-check"))}">${mag}×</button>`).join("")}</div>
    ${hoQty("reticle-cal", t("reticle-cal"), "", "count", "reticle-cal", `<input id="reticle-cal" value="${esc(reticleState.cal)}" inputmode="decimal" title="${esc(t("reticle-cal-tip"))}" autocomplete="off" spellcheck="false" />`)}
    <div class="ho-field" title="${esc(t("reticle-fov-tip"))}"><div class="head"><label for="reticle-fov">${esc(t("reticle-fov"))}</label></div><div class="ho-entry"><input id="reticle-fov" class="num" value="${esc(reticleState.fov)}" inputmode="decimal" title="${esc(t("reticle-fov-tip"))}" autocomplete="off" /></div></div>
    ${hoSelect("reticle-focal", t("reticle-focal"), `<option value="ffp" ${reticleState.focal === "ffp" ? "selected" : ""}>${esc(t("focal-ffp"))}</option><option value="sfp" ${reticleState.focal === "sfp" ? "selected" : ""}>${esc(t("focal-sfp"))}</option>`, t("reticle-focal-tip"))}
    ${hoSelect("reticle-target", t("impact-target"), `<option value="">${esc(t("reticle-no-target"))}</option>${targetOptions(reticleState.target)}`, t("impact-target-tip"))}
    <span title="${esc(t("reticle-range-tip"))}">${qtyField("reticle-range", t("range"), reticleState.rangeText, units().length, "length", null, null, "reticle-range")}</span>
    ${hoSelect("reticle-facing", t("facing"), `<option value="left" ${reticleState.facing === "left" ? "selected" : ""}>${esc(t("facing-left"))}</option><option value="right" ${reticleState.facing === "right" ? "selected" : ""}>${esc(t("facing-right"))}</option>`, t("facing-tip"))}
    ${hoSelect("reticle-ringing", t("ringing"), `<option value="primary" ${reticleState.ringing !== "alt" ? "selected" : ""}>${esc(t("ringing-primary"))}</option><option value="alt" ${reticleState.ringing === "alt" ? "selected" : ""}>${esc(t("ringing-alt"))}</option>`, t("ringing-tip"))}
    <label class="ho-check" title="${esc(t("reticle-hold-tip"))}"><input id="reticle-hold" type="checkbox" ${reticleState.showHold ? "checked" : ""} /> ${esc(t("reticle-hold"))}</label>
    <label class="ho-check" title="${esc(t("silhouette-tip"))}"><input id="reticle-silhouette" type="checkbox" ${reticleState.showSilhouette ? "checked" : ""} /> ${esc(t("silhouette"))}</label>`;
  const main = `<div class="ho-tabs" role="tablist" aria-label="${esc(t("reticle-title"))}">
      <button type="button" role="tab" data-reticle-pane="view" aria-selected="${reticlePane === "view"}">${esc(t("reticle-tab-view"))}</button>
      <button type="button" role="tab" data-reticle-pane="editor" aria-selected="${reticlePane === "editor"}">${esc(t("editor-title"))}</button>
    </div>
    <div id="reticle-view" class="pane${reticlePane === "view" ? "" : " is-hidden"}">
      <div class="grid2">
        <div id="reticle-face"></div>
        <section class="ho-card stack">
          <p id="reticle-readout" class="ho-hint"></p>
          <p id="reticle-warn" class="ho-badge warn hidden"></p>
          <div id="reticle-flags"></div>
          <dl class="ho-kv" id="reticle-kv"></dl>
        </section>
      </div>
    </div>
    <div id="reticle-editor-pane" class="pane${reticlePane === "editor" ? "" : " is-hidden"}">
      <section class="ho-card stack">
        <p class="ho-hint">${esc(t("editor-intro"))}</p>
        <div class="ho-field wide" title="${esc(t("editor-name-tip"))}"><div class="head"><label for="reticle-name">${esc(t("editor-name"))}</label></div><div class="ho-entry"><input id="reticle-name" title="${esc(t("editor-name-tip"))}" autocomplete="off" /></div></div>
        <div class="ho-fields">
          ${hoSelect("reticle-tool", t("editor-tool"), tools, t("editor-tool-tip"))}
          <div class="ho-field" title="${esc(t("editor-text-tip"))}"><div class="head"><label for="reticle-text">${esc(t("editor-text"))}</label></div><div class="ho-entry"><input id="reticle-text" value="1" title="${esc(t("editor-text-tip"))}" autocomplete="off" /></div></div>
          <div class="ho-field" title="${esc(t("editor-snap-tip"))}"><div class="head"><label for="reticle-snap">${esc(t("editor-snap"))}</label></div><div class="ho-entry"><input id="reticle-snap" class="num" value="${esc(reticleState.snap)}" inputmode="decimal" title="${esc(t("editor-snap-tip"))}" autocomplete="off" /></div></div>
          <div class="ho-field" title="${esc(t("editor-repeat-tip"))}"><div class="head"><label for="reticle-copies">${esc(t("editor-repeat"))}</label></div><div class="ho-entry"><input id="reticle-copies" class="num" value="3" inputmode="decimal" title="${esc(t("editor-repeat-tip"))}" autocomplete="off" /></div></div>
          <div class="ho-field" title="${esc(t("editor-dx-tip"))}"><div class="head"><label for="reticle-dx">${esc(t("editor-dx"))}</label></div><div class="ho-entry"><input id="reticle-dx" class="num" value="1" inputmode="decimal" title="${esc(t("editor-dx-tip"))}" autocomplete="off" /></div></div>
          <div class="ho-field" title="${esc(t("editor-dy-tip"))}"><div class="head"><label for="reticle-dy">${esc(t("editor-dy"))}</label></div><div class="ho-entry"><input id="reticle-dy" class="num" value="0" inputmode="decimal" title="${esc(t("editor-dy-tip"))}" autocomplete="off" /></div></div>
        </div>
        <div class="btnrow">
          <button type="button" class="ho-btn" id="reticle-mirror-x" title="${esc(t("editor-mirror-x-tip"))}">${esc(t("editor-mirror-x"))}</button>
          <button type="button" class="ho-btn" id="reticle-mirror-y" title="${esc(t("editor-mirror-y-tip"))}">${esc(t("editor-mirror-y"))}</button>
          <button type="button" class="ho-btn" id="reticle-repeat" title="${esc(t("editor-repeat-tip"))}">${esc(t("editor-repeat-apply"))}</button>
          <button type="button" class="ho-btn ghost" id="reticle-undo" title="${esc(t("editor-undo-tip"))}">${esc(t("editor-undo"))}</button>
          <button type="button" class="ho-btn primary" id="reticle-save" title="${esc(t("editor-save-tip"))}">${esc(t("editor-save"))}</button>
          <button type="button" class="ho-btn" id="reticle-export" title="${esc(t("editor-export-tip"))}">${esc(t("editor-export"))}</button>
        </div>
        <div class="ho-field" title="${esc(t("editor-import-tip"))}"><div class="head"><label for="reticle-import">${esc(t("editor-import"))}</label></div><input id="reticle-import" type="file" accept="application/json,.json" title="${esc(t("editor-import-tip"))}" /></div>
        <p id="reticle-notice" class="ho-hint">${esc(reticleNotice)}</p>
        <p class="ho-hint">${esc(t("reticle-pdf-note"))}</p>
      </section>
    </div>`;
  return frame({ rail, main, railLabel: t("nav-reticle") });
}

function ensureDraft() {
  if (reticleState.draft && reticleState.draft._picked === reticleState.id) return;
  const rows = (boot.reticles || []).concat((boot.equipment && boot.equipment.reticles) || []);
  const found = rows.find((item) => item.id === reticleState.id);
  const draft = found ? JSON.parse(JSON.stringify(found)) : {
    format: "holdover-reticle", version: 1, id: "", name: t("editor-new-name"),
    focal_plane: "ffp", unit: units().angle === "MOA" ? "MOA" : "mrad", calibrated_magnification: 10,
    licence: "user-drawn", source: "User drawn in Holdover.", elements: [],
  };
  draft._picked = reticleState.id;
  draft.elements = draft.elements || [];
  reticleState.draft = draft;
  reticleState.lineStart = null;
}

function bindReticle() {
  document.querySelectorAll("[data-reticle-pane]").forEach((button) => button.addEventListener("click", () => {
    reticlePane = button.dataset.reticlePane;
    document.querySelectorAll("[data-reticle-pane]").forEach((item) => item.setAttribute("aria-selected", item === button ? "true" : "false"));
    const view = document.getElementById("reticle-view");
    const editor = document.getElementById("reticle-editor-pane");
    if (view) view.classList.toggle("is-hidden", reticlePane !== "view");
    if (editor) editor.classList.toggle("is-hidden", reticlePane !== "editor");
  }));
  ensureDraft();
  const name = document.getElementById("reticle-name");
  if (name && reticleState.draft) name.value = reticleState.draft.name || "";
  const read = (id, assign) => {
    const node = document.getElementById(id);
    if (!node) return;
    node.addEventListener("input", () => { assign(node); scheduleReticle(); });
  };
  read("reticle-mag", (node) => { reticleState.mag = Number(node.value) || reticleState.mag; });
  read("reticle-fov", (node) => { reticleState.fov = Number(node.value) || reticleState.fov; });
  read("reticle-cal", (node) => { reticleState.cal = Number(node.value) || reticleState.cal; });
  read("reticle-range", (node) => { reticleState.rangeText = node.value; node.dataset.dirty = "1"; });
  read("reticle-snap", (node) => { reticleState.snap = Number(node.value) || 0; });
  read("reticle-name", (node) => { if (reticleState.draft) reticleState.draft.name = node.value; });
  ["reticle-pick", "reticle-focal", "reticle-target", "reticle-facing", "reticle-ringing", "reticle-tool"].forEach((id) => {
    const node = document.getElementById(id);
    if (!node) return;
    node.addEventListener("change", () => {
      if (id === "reticle-pick") { reticleState.id = node.value; reticleState.draft = null; ensureDraft(); if (name) name.value = reticleState.draft.name || ""; }
      if (id === "reticle-focal") reticleState.focal = node.value;
      if (id === "reticle-target") reticleState.target = node.value;
      if (id === "reticle-facing") reticleState.facing = node.value;
      if (id === "reticle-ringing") reticleState.ringing = node.value;
      if (id === "reticle-tool") reticleState.tool = node.value;
      scheduleReticle();
    });
  });
  document.querySelectorAll("[data-mag]").forEach((button) => button.addEventListener("click", () => {
    reticleState.mag = Number(button.dataset.mag);
    const input = document.getElementById("reticle-mag");
    if (input) input.value = String(reticleState.mag);
    scheduleReticle();
  }));
  const hold = document.getElementById("reticle-hold");
  if (hold) hold.addEventListener("change", () => { reticleState.showHold = hold.checked; scheduleReticle(); });
  const silhouette = document.getElementById("reticle-silhouette");
  if (silhouette) silhouette.addEventListener("change", () => { reticleState.showSilhouette = silhouette.checked; scheduleReticle(); });
  document.getElementById("reticle-mirror-x").addEventListener("click", () => { mirrorDraft("x"); scheduleReticle(); });
  document.getElementById("reticle-mirror-y").addEventListener("click", () => { mirrorDraft("y"); scheduleReticle(); });
  document.getElementById("reticle-repeat").addEventListener("click", () => { repeatDraft(); scheduleReticle(); });
  document.getElementById("reticle-undo").addEventListener("click", () => {
    if (reticleState.draft && reticleState.draft.elements.length) reticleState.draft.elements.pop();
    scheduleReticle();
  });
  document.getElementById("reticle-save").addEventListener("click", saveReticle);
  document.getElementById("reticle-export").addEventListener("click", () => {
    ensureDraft();
    downloadText("holdover-reticle.json", JSON.stringify(reticleState.draft, null, 2), "application/json");
  });
  document.getElementById("reticle-import").addEventListener("change", async (event) => {
    const file = event.target.files && event.target.files[0];
    if (!file) return;
    try {
      const parsed = JSON.parse(await file.text());
      parsed._picked = reticleState.id;
      reticleState.draft = parsed;
      scheduleReticle();
    } catch (error) {
      reticleNotice = String(error);
      const note = document.getElementById("reticle-notice");
      if (note) note.textContent = reticleNotice;
    }
  });
  scheduleReticle();
}

function scheduleReticle() {
  window.clearTimeout(reticleTimer);
  reticleTimer = window.setTimeout(drawReticle, 60);
}

function mirrorDraft(axis) {
  ensureDraft();
  const flip = (x, y) => axis === "x" ? [-x, y] : [x, -y];
  reticleState.draft.elements = (reticleState.draft.elements || []).map((mark) => {
    if (mark.kind === "line") {
      const a = flip(mark.x1, mark.y1);
      const b = flip(mark.x2, mark.y2);
      return Object.assign({}, mark, { x1: a[0], y1: a[1], x2: b[0], y2: b[1] });
    }
    if (mark.kind === "polyline") {
      return Object.assign({}, mark, { points: (mark.points || []).map((point) => flip(point[0], point[1])) });
    }
    if (mark.x !== undefined) {
      const at = flip(mark.x, mark.y);
      return Object.assign({}, mark, { x: at[0], y: at[1] });
    }
    return mark;
  });
}

function repeatDraft() {
  ensureDraft();
  const copies = Math.max(1, Math.min(40, Number(document.getElementById("reticle-copies").value) || 1));
  const dx = Number(document.getElementById("reticle-dx").value) || 0;
  const dy = Number(document.getElementById("reticle-dy").value) || 0;
  const base = reticleState.draft.elements.slice();
  const out = [];
  for (let index = 0; index < copies; index += 1) {
    base.forEach((mark) => out.push(shiftMark(mark, dx * index, dy * index)));
  }
  reticleState.draft.elements = out;
}

function shiftMark(mark, dx, dy) {
  const copy = Object.assign({}, mark);
  if (copy.kind === "line") { copy.x1 += dx; copy.y1 += dy; copy.x2 += dx; copy.y2 += dy; }
  else if (copy.kind === "polyline") copy.points = (copy.points || []).map((point) => [point[0] + dx, point[1] + dy]);
  else if (copy.x !== undefined) { copy.x += dx; copy.y += dy; }
  return copy;
}

async function saveReticle() {
  ensureDraft();
  const draft = JSON.parse(JSON.stringify(reticleState.draft));
  delete draft._picked;
  draft.licence = "user-drawn";
  draft.source = draft.source || "User drawn in Holdover.";
  const savedAlready = ((boot.equipment && boot.equipment.reticles) || []).some((item) => item.id === draft.id);
  if (!savedAlready) draft.id = "";
  try {
    const saved = await call("reticle_save", { body: draft });
    reticleNotice = t("editor-saved");
    reticleState.id = saved.id;
    reticleState.draft = null;
    boot = await call("bootstrap");
    render();
  } catch (error) {
    reticleNotice = String(error);
    const note = document.getElementById("reticle-notice");
    if (note) note.textContent = reticleNotice;
  }
}

async function drawReticle() {
  const host = document.getElementById("reticle-face");
  if (!host || page !== "reticle") return;
  ensureDraft();
  const draft = JSON.parse(JSON.stringify(reticleState.draft));
  delete draft._picked;
  const range = qtyPayload("reticle-range") || { text: reticleState.rangeText || "100 m", unit: units().length };
  const body = {
    reticle: draft,
    focal_plane: reticleState.focal,
    calibrated_magnification: Number(reticleState.cal) || 10,
    magnification: Number(reticleState.mag) || 10,
    apparent_fov_deg: Number(reticleState.fov) || 22,
    viewport_px: 1000,
    range,
    target_id: reticleState.target,
    facing: reticleState.facing,
    elevation_mrad: 0,
    windage_mrad: 0,
    lead_mrad: 0,
  };
  if (reticleState.showHold && lastSolve && lastSolve.elevation) {
    body.elevation_mrad = Number(lastSolve.elevation.mrad) || 0;
    body.windage_mrad = Number(lastSolve.windage.mrad) || 0;
    if (lastSolve.lead && lastSolve.lead.lead) body.lead_mrad = Number(lastSolve.lead.lead.mrad) || 0;
  }
  try {
    reticleScene = await call("reticle_scene", { body });
    host.innerHTML = reticleSvg(reticleScene);
    const readout = document.getElementById("reticle-readout");
    if (readout) readout.textContent = reticleReadout(reticleScene);
    const kv = document.getElementById("reticle-kv");
    if (kv) {
      const one = Number(reticleScene.nominal_one_true_mrad);
      const ring = reticleScene.target && reticleScene.target.rings && reticleScene.target.rings[0];
      const ringText = ring && reticleScene.cm_per_mrad
        ? `${(Number(ring.diameter_cm) / Number(reticleScene.cm_per_mrad)).toFixed(2)} mrad`
        : t("missing-not-set");
      kv.innerHTML = `<dt>${esc(t("reticle-scale"))}</dt><dd>${Number.isFinite(one) ? esc(one.toFixed(3)) : ""}</dd><span class="ho-unit">mrad</span>
        <dt>${esc(t("reticle-factor"))}</dt><dd>${esc(Number(reticleScene.sfp_factor).toFixed(3))}</dd><span class="ho-unit"></span>
        <dt>${esc(t("reticle-ring-px"))}</dt><dd>${esc(ringText)}</dd><span class="ho-unit"></span>`;
    }
    const warn = document.getElementById("reticle-warn");
    if (warn) {
      warn.textContent = reticleScene.sfp_warning ? t("sfp-warning") : "";
      warn.classList.toggle("hidden", !reticleScene.sfp_warning);
    }
    const flags = document.getElementById("reticle-flags");
    if (flags) flags.innerHTML = reticleScene.target ? flagHtml(reticleScene.target.flags) : "";
    const svg = host.querySelector("svg");
    if (svg) svg.addEventListener("click", reticleClick);
  } catch (error) {
    host.innerHTML = `<p class="error">${esc(error)}</p>`;
  }
}

function reticleReadout(scene) {
  const one = Number(scene.nominal_one_true_mrad);
  const unit = scene.unit || "mrad";
  let text = `${t("reticle-scale")} ${one.toFixed(3)} mrad ${t("reticle-per")} 1 ${unit}. ${t("reticle-factor")} ${Number(scene.sfp_factor).toFixed(3)}.`;
  const target = scene.target;
  const ring = target && target.rings && target.rings[0];
  if (ring && scene.cm_per_mrad) {
    const px = Number(ring.diameter_cm) / Number(scene.cm_per_mrad) * Number(scene.px_per_mrad);
    text += ` ${t("reticle-ring-px")} ${px.toFixed(1)} px.`;
  }
  return text;
}

function reticleSvg(scene) {
  const size = Number(scene.viewport_px) || 1000;
  const c = size / 2;
  const px = (x, y) => [c + x * scene.px_per_mrad, c - y * scene.px_per_mrad];
  const stepTrue = Math.abs(Number(scene.nominal_one_true_mrad) || 1) * (Number(reticleState.snap) || 1);
  const stepPx = stepTrue * Number(scene.px_per_mrad);
  let grid = "";
  if (stepPx >= 12 && stepPx < size) {
    for (let at = c + stepPx; at < size; at += stepPx) {
      const mirror = 2 * c - at;
      grid += gridLine(at, c, size, true);
      if (mirror > 0 && Math.abs(mirror - at) > 0.5) grid += gridLine(mirror, c, size, true);
    }
    for (let at = c + stepPx; at < size; at += stepPx) {
      const mirror = 2 * c - at;
      grid += gridLine(at, c, size, false);
      if (mirror > 0 && Math.abs(mirror - at) > 0.5) grid += gridLine(mirror, c, size, false);
    }
  }
  let marks = "";
  (scene.elements || []).forEach((mark) => { marks += markSvg(mark, px); });
  let target = "";
  if (scene.target) {
    const aim = scene.aim_mrad || { x: 0, y: 0 };
    const cm = Number(scene.cm_per_mrad) || 1;
    const to = (x, y) => px(x / cm + aim.x, y / cm + aim.y);
    target = targetMarks(scene.target, to, reticleState.ringing, reticleState.showSilhouette);
  }
  const centre = `<line stroke="var(--accent)" stroke-width="1.5" x1="${(c - 12).toFixed(1)}" x2="${(c + 12).toFixed(1)}" y1="${c}" y2="${c}" />
    <line stroke="var(--accent)" stroke-width="1.5" y1="${(c - 12).toFixed(1)}" y2="${(c + 12).toFixed(1)}" x1="${c}" x2="${c}" />
    <text fill="var(--text)" x="16" y="28">${esc(t(reticleState.showHold ? "reticle-hold-point" : "reticle-centre"))}</text>`;
  const overlays = holdMarks(scene, px);
  return `<svg class="reticle" viewBox="0 0 ${size} ${size}" role="img" aria-label="${esc(t("reticle-title"))}">
    <rect width="${size}" height="${size}" fill="var(--background)" />
    ${grid}${target}${marks}${centre}${overlays}
  </svg>`;
}

function holdMarks(scene, px) {
  if (!reticleState.showHold) return "";
  const bits = [];
  const wind = Number(scene.windage_mrad) || 0;
  const lead = Number(scene.lead_mrad) || 0;
  if (Math.abs(wind) > 0.02) bits.push(overlayDot(px(wind, 0), "var(--chart-1)", t("reticle-wind-dot")));
  if (Math.abs(lead) > 0.02) bits.push(overlayDot(px(lead, 0), "var(--chart-2)", t("reticle-lead-point")));
  return bits.join("");
}

function overlayDot(at, color, label) {
  return `<circle cx="${at[0].toFixed(1)}" cy="${at[1].toFixed(1)}" r="6" fill="${color}" />
    <text fill="var(--text)" x="${(at[0] + 10).toFixed(1)}" y="${(at[1] - 8).toFixed(1)}">${esc(label)}</text>`;
}

function gridLine(at, c, size, vertical) {
  if (vertical) return `<line stroke="var(--border)" stroke-width="1" x1="${at.toFixed(1)}" x2="${at.toFixed(1)}" y1="0" y2="${size}" />`;
  return `<line stroke="var(--border)" stroke-width="1" y1="${at.toFixed(1)}" y2="${at.toFixed(1)}" x1="0" x2="${size}" />`;
}

function markSvg(mark, px) {
  const ink = "var(--text)";
  if (mark.kind === "line") {
    const a = px(mark.x1, mark.y1);
    const b = px(mark.x2, mark.y2);
    const width = Math.max(1, Math.min(8, Math.abs(mark.weight || 0.04) * (reticleScene.px_per_mrad || 20)));
    return `<line stroke="${ink}" stroke-width="${width.toFixed(2)}" stroke-linecap="round" x1="${a[0].toFixed(1)}" y1="${a[1].toFixed(1)}" x2="${b[0].toFixed(1)}" y2="${b[1].toFixed(1)}" />`;
  }
  if (mark.kind === "hash") {
    const angle = Number(mark.angle || 0) * Math.PI / 180;
    const len = Number(mark.length || 0.2) / 2;
    const a = px(mark.x - Math.cos(angle) * len, mark.y - Math.sin(angle) * len);
    const b = px(mark.x + Math.cos(angle) * len, mark.y + Math.sin(angle) * len);
    return `<line stroke="${ink}" stroke-width="1.5" x1="${a[0].toFixed(1)}" y1="${a[1].toFixed(1)}" x2="${b[0].toFixed(1)}" y2="${b[1].toFixed(1)}" />`;
  }
  if (mark.kind === "dot" || mark.kind === "circle") {
    const at = px(mark.x, mark.y);
    const radius = Math.max(1, Number(mark.diameter || 0.2) / 2 * (reticleScene.px_per_mrad || 20));
    const fill = mark.kind === "dot" ? `fill="${ink}"` : `fill="none"`;
    return `<circle cx="${at[0].toFixed(1)}" cy="${at[1].toFixed(1)}" r="${radius.toFixed(1)}" ${fill} stroke="${ink}" stroke-width="1.5" />`;
  }
  if (mark.kind === "polyline") {
    const d = (mark.points || []).map((point, index) => {
      const at = px(point[0], point[1]);
      return `${index === 0 ? "M" : "L"} ${at[0].toFixed(1)} ${at[1].toFixed(1)}`;
    }).join(" ");
    return `<path d="${d}${mark.closed ? " Z" : ""}" fill="none" stroke="${ink}" stroke-width="1.5" />`;
  }
  if (mark.kind === "text") {
    const at = px(mark.x, mark.y);
    return `<text fill="${ink}" x="${at[0].toFixed(1)}" y="${at[1].toFixed(1)}">${esc(mark.text || "")}</text>`;
  }
  return "";
}

function targetMarks(spec, to, ringing, showSilhouette) {
  let body = "";
  (spec.paths || []).forEach((path) => {
    if (!showSilhouette && path.role === "silhouette") return;
    const points = path.points || [];
    if (!points.length) return;
    const stroke = path.placeholder ? "var(--warning)" : "var(--text)";
    const dash = path.placeholder ? 'stroke-dasharray="8 5"' : "";
    if (path.closed === false) {
      let d = "";
      for (let i = 0; i + 1 < points.length; i += 2) {
        const a = to(Number(points[i][0]), Number(points[i][1]));
        const b = to(Number(points[i + 1][0]), Number(points[i + 1][1]));
        d += `M ${a[0].toFixed(1)} ${a[1].toFixed(1)} L ${b[0].toFixed(1)} ${b[1].toFixed(1)} `;
      }
      body += `<path d="${d}" fill="none" stroke="${stroke}" stroke-width="1.5" ${dash} />`;
      return;
    }
    const d = points.map((point, index) => {
      const at = to(Number(point[0]), Number(point[1]));
      return `${index === 0 ? "M" : "L"} ${at[0].toFixed(1)} ${at[1].toFixed(1)}`;
    }).join(" ") + " Z";
    body += `<path d="${d}" fill="none" stroke="${stroke}" stroke-width="1.75" ${dash} />`;
  });
  (spec.rings || []).forEach((ring) => {
    const radius = Number(ring.diameter_cm) / 2;
    const edge = ringing === "alt" ? to(radius, 0) : to(radius * Math.SQRT1_2, radius * Math.SQRT1_2);
    const centre = to(0, 0);
    const r = Math.abs(edge[0] - centre[0]);
    const label = (ringing === "alt" && ring.label_alt) || ring.label_primary || "";
    body += `<circle cx="${centre[0].toFixed(1)}" cy="${centre[1].toFixed(1)}" r="${r.toFixed(1)}" fill="none" stroke="var(--text)" stroke-width="1.5" />`;
    if (label) body += `<text fill="var(--text)" x="${(edge[0] + 4).toFixed(1)}" y="${(edge[1] - 4).toFixed(1)}">${esc(label)}</text>`;
  });
  return body;
}

function reticleClick(event) {
  if (!reticleScene || !reticleState.draft) return;
  const svg = event.currentTarget;
  const point = svgPoint(svg, event);
  const c = Number(reticleScene.viewport_px) / 2;
  const trueX = (point.x - c) / reticleScene.px_per_mrad;
  const trueY = (c - point.y) / reticleScene.px_per_mrad;
  const factor = Number(reticleScene.sfp_factor) || 1;
  let x = trueX / factor;
  let y = trueY / factor;
  if (reticleScene.unit === "MOA") {
    x = x * 10.8 / Math.PI;
    y = y * 10.8 / Math.PI;
  }
  const step = Number(reticleState.snap) || 0;
  if (step > 0) {
    x = Math.round(x / step) * step;
    y = Math.round(y / step) * step;
  }
  const tool = reticleState.tool;
  if (tool === "line") {
    if (!reticleState.lineStart) {
      reticleState.lineStart = [x, y];
      reticleNotice = t("editor-line-next");
      const note = document.getElementById("reticle-notice");
      if (note) note.textContent = reticleNotice;
      return;
    }
    const start = reticleState.lineStart;
    reticleState.lineStart = null;
    reticleState.draft.elements.push({ kind: "line", x1: start[0], y1: start[1], x2: x, y2: y, weight: 0.04 });
  } else if (tool === "hash") {
    reticleState.draft.elements.push({ kind: "hash", x, y, length: step > 0 ? step : 0.5, angle: 90, weight: 0.04 });
  } else if (tool === "dot") {
    reticleState.draft.elements.push({ kind: "dot", x, y, diameter: Math.max(0.05, step > 0 ? step * 0.4 : 0.2) });
  } else if (tool === "circle") {
    reticleState.draft.elements.push({ kind: "circle", x, y, diameter: Math.max(0.1, step > 0 ? step * 2 : 1) });
  } else if (tool === "text") {
    const text = (document.getElementById("reticle-text") || {}).value || "1";
    reticleState.draft.elements.push({ kind: "text", x, y, text });
  }
  drawReticle();
}

function svgPoint(svg, event) {
  const matrix = svg.getScreenCTM();
  const point = svg.createSVGPoint();
  point.x = event.clientX;
  point.y = event.clientY;
  if (!matrix) return point;
  return point.matrixTransform(matrix.inverse());
}

function targetListButtons() {
  const library = (boot.targets && boot.targets.library) || [];
  const saved = (boot.equipment && boot.equipment.targets) || [];
  const seen = new Set();
  return library.concat(saved).filter((item) => {
    if (!item || seen.has(item.id)) return false;
    seen.add(item.id);
    return true;
  }).map((item) => {
    const badge = (item.flags || []).map((flag) => flag.code).filter(Boolean).slice(0, 2).map((code) => `<span class="ho-badge warn">${esc(code)}</span>`).join(" ");
    return `<button type="button" data-target-list="${esc(item.id)}" aria-selected="${item.id === targetPickId()}"><span>${esc(item.name)}</span><span class="sub">${badge}</span></button>`;
  }).join("");
}

function targetRingsTable(spec) {
  const rings = ((spec && spec.rings) || []).filter((ring) => Number(ring.diameter_cm) > 0);
  if (!rings.length) return `<p class="ho-hint">${esc(t("target-no-points"))}</p>`;
  const rows = rings.map((ring) => `<tr><td>${esc(ring.label_primary || ring.label_alt || "")}</td><td>${esc(String(ring.diameter_cm))}</td><td>${ring.verified === false ? esc(t("missing-not-set")) : ""}</td></tr>`).join("");
  return `<table class="ho-table"><thead><tr><th class="text">${esc(t("label"))}</th><th>cm</th><th class="text"></th></tr></thead><tbody>${rows}</tbody></table>`;
}

function targetsHtml() {
  const pages = (boot.card_pages || []).map((item) => `<option value="${esc(item.id)}" ${reticleState.page === item.id ? "selected" : ""}>${esc(item.name)}</option>`).join("");
  const spec = currentTarget();
  const rail = `<h2>${esc(t("targets-page-title"))}</h2>
    <p class="ho-hint">${esc(t("targets-intro"))}</p>
    <div class="ho-list">${targetListButtons()}</div>
    <select id="target-pick" class="hidden" title="${esc(t("impact-target-tip"))}">${targetOptions(targetPickId())}</select>`;
  const main = `<div class="grid2">
      <section class="ho-card stack">
        <div class="ho-fields">
          ${hoSelect("target-facing", t("facing"), `<option value="left" ${targetFacing !== "right" ? "selected" : ""}>${esc(t("facing-left"))}</option><option value="right" ${targetFacing === "right" ? "selected" : ""}>${esc(t("facing-right"))}</option>`, t("facing-tip"))}
          ${hoSelect("target-ringing", t("ringing"), `<option value="primary" ${targetRinging !== "alt" ? "selected" : ""}>${esc(t("ringing-primary"))}</option><option value="alt" ${targetRinging === "alt" ? "selected" : ""}>${esc(t("ringing-alt"))}</option>`, t("ringing-tip"))}
          ${hoSelect("target-page", t("card-size"), pages, t("card-size"))}
        </div>
        <label class="ho-check" title="${esc(t("silhouette-tip"))}"><input id="target-silhouette" type="checkbox" ${targetSilhouette ? "checked" : ""} /> ${esc(t("silhouette"))}</label>
        <div id="target-flags">${spec ? flagHtml(spec.flags) : ""}</div>
        <p class="ho-hint">${spec ? esc(spec.subtitle || "") : ""} ${spec && spec.distance_note ? "· " + esc(spec.distance_note) : ""}</p>
        <div id="target-face">${spec ? faceSvg(spec, { showSilhouette: true, ringing: "primary", label: spec.name }) : ""}</div>
        ${spec ? targetLegend(spec) : ""}
        <h3>${esc(t("target-edit-title"))}</h3>
        <p class="ho-hint">${esc(t("target-edit-intro"))}</p>
        <div id="target-editor">${targetEditorHtml(spec)}</div>
        <div class="btnrow">
          <button type="button" class="ho-btn primary" id="target-save" title="${esc(t("target-save-tip"))}">${esc(t("target-save"))}</button>
          <button type="button" class="ho-btn" id="target-pdf" title="${esc(t("target-pdf-tip"))}">${esc(t("target-pdf"))}</button>
        </div>
        <p id="target-notice" class="ho-hint">${esc(targetNotice)}</p>
        <h3>${esc(t("svg-title"))}</h3>
        <p class="ho-hint">${esc(t("svg-intro"))}</p>
        <div class="ho-field" title="${esc(t("svg-file-tip"))}"><div class="head"><label for="svg-file">${esc(t("svg-file"))}</label></div><input id="svg-file" type="file" accept=".svg,image/svg+xml" title="${esc(t("svg-file-tip"))}" /></div>
        <div class="ho-fields">
          <span title="${esc(t("svg-width-tip"))}">${qtyField("svg-width", t("svg-width"), "20 cm", units().small, "length")}</span>
          <span title="${esc(t("svg-height-tip"))}">${qtyField("svg-height", t("svg-height"), "20 cm", units().small, "length")}</span>
        </div>
        <button type="button" class="ho-btn" id="svg-save" title="${esc(t("svg-save-tip"))}">${esc(t("svg-save"))}</button>
      </section>
      <section class="ho-card stack">
        <header><h2>${esc((spec && spec.name) || t("nav-targets"))}</h2></header>
        ${targetRingsTable(spec)}
        <header><h2>${esc(t("open-items"))}</h2></header>
        <p class="ho-hint">${esc(t("open-items-intro"))}</p>
        <div id="open-items">${openItemsHtml()}</div>
      </section>
    </div>`;
  return frame({ rail, main, body: "with-list", railLabel: t("nav-targets") });
}

let targetPick = "sjf-algfigur";
let targetFacing = "left";
let targetRinging = "primary";
let targetSilhouette = true;
let targetDraft = null;

function targetPickId() { return targetPick; }
function targetFacingValue() { return targetFacing; }

function currentTarget() {
  if (targetDraft && targetDraft.id === targetPick) return targetDraft;
  return libraryTarget(targetPick);
}

function openItemsHtml() {
  const spec = currentTarget();
  const flags = ((spec && spec.flags) || []).filter((flag) => flag.code !== "T1");
  if (!flags.length) return `<p class="ho-hint">${esc(t("open-items-none"))}</p>`;
  return flagHtml(flags);
}

function targetEditorHtml(spec) {
  if (!spec) return "";
  const unit = units().small || "cm";
  const paths = (spec.paths || []).map((path, index) => `<label class="field" title="${esc(t("target-points-tip"))}">${esc(t("target-points"))} ${esc(path.role)} (${esc(unit)})
      <textarea id="path-${index}" data-role="${esc(path.role)}" rows="6" title="${esc(t("target-points-tip"))}">${esc(pointsText(path.points))}</textarea>
    </label>`).join("");
  const vital = (spec.flags || []).some((flag) => flag.code === "T6")
    ? `<label class="field" title="${esc(t("vital-tip"))}">${esc(t("vital"))} (${esc(unit)})
        <input id="vital-diameter" inputmode="decimal" title="${esc(t("vital-tip"))}" placeholder="${esc(t("vital-empty"))}" />
      </label>`
    : "";
  const label = `<label class="field" title="${esc(t("target-label-tip"))}">${esc(t("editor-name"))}
      <input id="target-label" value="${esc(spec.name || "")}" title="${esc(t("target-label-tip"))}" />
    </label>`;
  return label + vital + paths + (paths ? "" : `<p class="hint">${esc(t("target-no-points"))}</p>`);
}

function pointsText(points) {
  const unit = units().small || "cm";
  return (points || []).map((point) => `${formatLocal(Number(point[0]) / 100, "length", unit)} ${formatLocal(Number(point[1]) / 100, "length", unit)}`).join("\n");
}

function bindTargets() {
  document.querySelectorAll("[data-target-list]").forEach((button) => button.addEventListener("click", () => {
    const select = document.getElementById("target-pick");
    if (!select) return;
    select.value = button.dataset.targetList;
    select.dispatchEvent(new Event("change"));
  }));
  const pick = document.getElementById("target-pick");
  if (pick) pick.addEventListener("change", () => {
    targetPick = pick.value;
    targetDraft = null;
    render();
  });
  ["target-facing", "target-ringing"].forEach((id) => {
    const node = document.getElementById(id);
    if (!node) return;
    node.addEventListener("change", () => {
      if (id === "target-facing") targetFacing = node.value;
      if (id === "target-ringing") targetRinging = node.value;
      paintTarget();
    });
  });
  const silhouette = document.getElementById("target-silhouette");
  if (silhouette) silhouette.addEventListener("change", () => { targetSilhouette = silhouette.checked; paintTarget(); });
  document.querySelectorAll("#target-editor textarea").forEach((box) => {
    box.addEventListener("input", () => { applyTargetDraft(); paintTarget(); });
  });
  const vital = document.getElementById("vital-diameter");
  if (vital) vital.addEventListener("input", () => { applyTargetDraft(); paintTarget(); });
  document.getElementById("target-save").addEventListener("click", saveTarget);
  document.getElementById("target-pdf").addEventListener("click", openTargetPdf);
  document.getElementById("svg-save").addEventListener("click", saveSvgTarget);
  paintTarget();
}

function applyTargetDraft() {
  const base = libraryTarget(targetPick);
  if (!base) return;
  const draft = JSON.parse(JSON.stringify(base));
  document.querySelectorAll("#target-editor textarea").forEach((box) => {
    const role = box.dataset.role;
    const points = parsePointText(box.value);
    const path = (draft.paths || []).find((item) => item.role === role);
    if (path && points.length) {
      path.points = points;
      path.placeholder = true;
      path.for_scoring = false;
    }
  });
  const vital = document.getElementById("vital-diameter");
  if (vital && String(vital.value || "").trim()) {
    const cm = toCm(Number(String(vital.value).replace(",", ".")), units().small || "cm");
    if (cm > 0) {
      draft.rings = (draft.rings || []).filter((ring) => ring.label_primary !== "vital");
      draft.rings.push({ diameter_cm: cm, label_primary: "vital", label_alt: "", verified: false });
    }
  }
  draft.id = targetPick;
  targetDraft = draft;
}

function parsePointText(text) {
  const unit = units().small || "cm";
  const points = [];
  String(text || "").split(/\n/).forEach((line) => {
    const parts = line.trim().split(/[\s,]+/).filter(Boolean);
    if (parts.length < 2) return;
    const x = toCm(Number(parts[0].replace(",", ".")), unit);
    const y = toCm(Number(parts[1].replace(",", ".")), unit);
    if (Number.isFinite(x) && Number.isFinite(y)) points.push([x, y]);
  });
  return points;
}

function paintTarget() {
  const host = document.getElementById("target-face");
  const spec = currentTarget();
  if (!host || !spec) return;
  const draw = JSON.parse(JSON.stringify(spec));
  if (targetFacing === "right") {
    (draw.paths || []).forEach((path) => (path.points || []).forEach((point) => { point[0] = -point[0]; }));
  }
  host.innerHTML = faceSvg(draw, { showSilhouette: targetSilhouette, ringing: targetRinging, label: draw.name });
  const flags = document.getElementById("target-flags");
  if (flags) flags.innerHTML = flagHtml(draw.flags);
}

async function saveTarget() {
  applyTargetDraft();
  const draft = targetDraft || libraryTarget(targetPick);
  if (!draft) return;
  const saved = ((boot.equipment && boot.equipment.targets) || []).find((item) => item.id === targetPick);
  const vital = document.getElementById("vital-diameter");
  const body = {
    base_id: saved ? (saved.base_id || "") : targetPick,
    id: saved ? saved.id : "",
    label: (document.getElementById("target-label") || {}).value || draft.name,
    paths: draft.paths || [],
  };
  if (vital && String(vital.value || "").trim()) {
    body.vital_diameter_cm = toCm(Number(String(vital.value).replace(",", ".")), units().small || "cm");
  }
  try {
    const stored = await call("target_save", { body });
    targetNotice = t("target-saved");
    targetPick = stored.id;
    targetDraft = null;
    boot = await call("bootstrap");
    render();
  } catch (error) {
    targetNotice = String(error);
    const note = document.getElementById("target-notice");
    if (note) note.textContent = targetNotice;
  }
}

async function openTargetPdf() {
  const spec = currentTarget();
  const notes = ((spec && spec.flags) || []).map((flag) => `${t("badge-" + flag.kind)}: ${t(flag.detail)}`);
  notes.push(t("target-print-scale"));
  const draw = spec ? JSON.parse(JSON.stringify(spec)) : { paths: [], rings: [] };
  if (targetFacing === "right") {
    (draw.paths || []).forEach((path) => (path.points || []).forEach((point) => { point[0] = -point[0]; }));
  }
  if (!targetSilhouette) draw.paths = (draw.paths || []).filter((path) => path.role !== "silhouette");
  try {
    const result = await call("target_pdf", { body: {
      target_id: targetPick,
      page: (document.getElementById("target-page") || {}).value || "a4",
      geometry_override: true,
      paths: (draw.paths || []).map((path) => ({
        points: path.points || [],
        closed: path.closed !== false,
        dashed: !!path.placeholder,
      })),
      rings_cm: (draw.rings || []).map((ring) => Number(ring.diameter_cm)).filter((value) => value > 0),
      notes,
    } });
    const scale = result.scale_denominator === 1 ? t("target-scale-one") : `${t("target-scale")} 1:${result.scale_denominator}`;
    targetNotice = `${result.opened ? t("card-pdf-opened") : t("card-pdf-saved")} ${scale}`;
    const note = document.getElementById("target-notice");
    if (note) note.textContent = targetNotice;
  } catch (error) {
    targetNotice = String(error);
    const note = document.getElementById("target-notice");
    if (note) note.textContent = targetNotice;
  }
}

async function saveSvgTarget() {
  const file = document.getElementById("svg-file").files && document.getElementById("svg-file").files[0];
  if (!file) return;
  const text = await file.text();
  try {
    const stored = await call("target_save", { body: {
      base_id: "custom",
      label: file.name.replace(/\.svg$/i, ""),
      svg: text,
      width_cm: qtyToCm("svg-width") || 20,
      height_cm: qtyToCm("svg-height") || 20,
    } });
    targetNotice = t("svg-saved");
    targetPick = stored.id;
    targetDraft = null;
    boot = await call("bootstrap");
    render();
  } catch (error) {
    targetNotice = String(error);
    const note = document.getElementById("target-notice");
    if (note) note.textContent = targetNotice;
  }
}

document.addEventListener("click", (event) => {
  const go = event.target.closest && event.target.closest("[data-go]");
  if (go) {
    page = go.dataset.go;
    render();
    return;
  }
  const button = event.target.closest && event.target.closest("#impact-svg, #impact-png");
  if (!button) return;
  if (button.id === "impact-svg") {
    const svg = document.querySelector("#impact-body svg");
    if (svg) downloadText("holdover-impact.svg", new XMLSerializer().serializeToString(svg), "image/svg+xml");
  } else {
    downloadPng("holdover-impact.png", "#impact-body svg");
  }
});

function ensureCompare() {
  if (compareReady) return;
  compareReady = true;
  const active = boot.settings.active_profile_id;
  if (active) compareIds = [active];
  const range = parseLocal(boot.settings.last_range || "300 m", "length", units().length);
  compareForm.rangeSi = range ? range.si : 300;
  const wind = parseLocal(boot.settings.last_wind_speed || "4 mps", "velocity", units().velocity);
  compareForm.windSi = wind ? wind.si : 4;
  compareForm.direction = boot.settings.last_wind_direction || "9oc";
}

function compareFlags() {
  if (!compareEffects) {
    compareEffects = Object.assign({ incline: true, spin_drift: true, coriolis: false, cant: false, aerodynamic_jump: false }, boot.settings.effects || {});
  }
  return compareEffects;
}

function compareHtml() {
  ensureCompare();
  const profiles = boot.equipment.profiles || [];
  const picks = profiles.map((item) => `<label class="ho-check"><input type="checkbox" class="compare-pick" value="${esc(item.id)}" ${compareIds.includes(item.id) ? "checked" : ""} /> ${esc(item.label)}</label>`).join("");
  const flags = compareFlags();
  const effect = (name, key) => `<label class="ho-check"><input type="checkbox" data-compare-effect="${name}" ${flags[name] ? "checked" : ""} /> ${esc(t(key))}</label>`;
  const rail = `<h2>${esc(t("nav-compare"))}</h2>
    <p class="ho-hint">${esc(t("compare-intro"))}</p>
    <div class="checks">${picks || `<p class="ho-hint">${esc(t("compare-empty"))}</p>`}</div>
    ${qtyField("compare-range", t("range"), "", units().length, "length", compareForm.rangeSi, null, "compare-range")}
    ${qtyField("compare-wind", t("wind-speed"), "", units().velocity, "velocity", compareForm.windSi, null, "compare-wind")}
    ${windClockField(compareForm.direction || "9oc", { id: "compare-direction", stepper: "compare-direction", key: false })}
    <div class="checks">
      ${effect("incline", "effect-incline")}
      ${effect("spin_drift", "effect-spin")}
      ${effect("coriolis", "effect-coriolis")}
      ${effect("cant", "effect-cant")}
      ${effect("aerodynamic_jump", "effect-jump")}
    </div>
    <p class="ho-hint">${esc(boot.spin_provenance || "")}</p>
    <p class="ho-hint">${esc(t("jump-note"))}</p>`;
  return frame({ rail, main: `<div id="compare-result"></div>`, railLabel: t("nav-compare") });
}

function bindCompare() {
  document.querySelectorAll(".compare-pick").forEach((box) => {
    box.addEventListener("change", () => {
      const id = box.value;
      if (box.checked) {
        if (compareIds.length >= 6) {
          box.checked = false;
          compareError = t("compare-limit");
        } else if (!compareIds.includes(id)) compareIds.push(id);
      } else compareIds = compareIds.filter((item) => item !== id);
      scheduleCompare(0);
    });
  });
  document.querySelectorAll("[data-compare-effect]").forEach((box) => {
    box.addEventListener("change", () => {
      compareFlags()[box.dataset.compareEffect] = box.checked;
      scheduleCompare(0);
    });
  });
  const direction = document.getElementById("compare-direction");
  if (direction) direction.addEventListener("input", () => {
    compareForm.direction = direction.value;
    scheduleCompare(150);
  });
}

function rememberCompareNumbers() {
  const range = document.getElementById("compare-range");
  if (range) {
    const parsed = parseLocal(range.value, "length", range.dataset.unit);
    if (parsed) {
      compareForm.rangeSi = parsed.si;
      range.dataset.si = String(parsed.si);
    }
  }
  const wind = document.getElementById("compare-wind");
  if (wind) {
    const parsed = parseLocal(wind.value, "velocity", wind.dataset.unit);
    if (parsed) {
      compareForm.windSi = parsed.si;
      wind.dataset.si = String(parsed.si);
    }
  }
}

function scheduleCompare(delay) {
  window.clearTimeout(compareTimer);
  compareTimer = window.setTimeout(runCompare, delay);
}

function compareBody() {
  const air = boot.settings.current_air || {};
  return {
    profile_ids: compareIds.slice(0, 6),
    range: qtyPayload("compare-range"),
    wind_speed: qtyPayload("compare-wind"),
    wind_direction: (document.getElementById("compare-direction") || {}).value || compareForm.direction || "9oc",
    inclination: storedQuantity(boot.settings.last_inclination, "deg"),
    current_air: {
      temperature: air.temperature || { text: "15 C", unit: "C" },
      pressure: air.pressure || { text: "1013.25 hPa", unit: "hPa" },
      pressure_kind: air.pressure_kind || "station",
      humidity: typeof air.humidity === "string" ? air.humidity : (air.humidity_text || "0%"),
      altitude: air.altitude || { text: "0 m", unit: "m" },
    },
    effects: compareFlags(),
    angular: boot.settings.angular || "mrad",
  };
}

async function runCompare() {
  const node = document.getElementById("compare-result");
  if (!node) return;
  if (!compareIds.length) {
    const none = !(boot.equipment.profiles || []).length;
    node.innerHTML = none ? emptyCta("compare-empty") : `<p class="ho-hint">${esc(t("compare-need"))}</p>`;
    return;
  }
  try {
    compareResult = await call("compare", { body: compareBody() });
    compareError = "";
    node.innerHTML = compareResultHtml(compareResult);
  } catch (error) {
    compareError = String(error);
    node.innerHTML = `<p class="error">${esc(compareError)}</p>` + (compareResult ? `<div class="hold stale">${compareResultHtml(compareResult)}</div>` : "");
  }
}

function missingText(value, kind) {
  const text = value == null ? "" : String(value).trim();
  if (text && text !== "—" && text !== "-" && text !== "–") return null;
  if (kind === "scope" || kind === "click") return t("missing-no-scope");
  if (kind === "twist") return t("missing-no-twist");
  return t("missing-not-set");
}

function splitMeasure(value, digits) {
  const missing = missingText(value);
  if (missing) return { num: missing, unit: "" };
  const pretty = prettyPhrase(String(value).trim());
  const match = pretty.match(/^(.*\d)\s+([^\d].*)$/);
  if (!match) return { num: pretty, unit: "" };
  const number = Number(match[1].replace(",", "."));
  const num = digits != null && Number.isFinite(number) ? number.toFixed(digits) : match[1];
  return { num, unit: match[2] };
}

function compareCell(parts) {
  return `<td>${esc(parts.num)}</td><td class="unit">${esc(parts.unit)}</td>`;
}

function compareTable(rows, caption, pairs) {
  const head = rows.map((row) => `<th colspan="2">${esc(row.label)}</th>`).join("");
  const body = pairs.map(([label, pick]) => `<tr><th class="text">${esc(label)}</th>${rows.map((row) => compareCell(pick(row))).join("")}</tr>`).join("");
  return `<section class="ho-card flush"><header><h2>${esc(caption)}</h2></header><div class="ho-table-wrap"><table class="ho-table ho-compare"><thead><tr><th></th>${head}</tr></thead><tbody>${body}</tbody></table></div></section>`;
}

function holdMeasure(node) {
  if (!node || missingText(node.primary)) return { num: t("missing-not-set"), unit: "" };
  const shown = figureShown(node);
  const dir = node.direction && node.direction !== "—" ? `${node.direction} ` : "";
  return { num: `${dir}${shown.num}`.trim(), unit: shown.unit };
}

function clickCount(node) {
  if (!node || node.clicks == null || node.clicks === "") return null;
  const count = Math.abs(Number(node.clicks));
  return Number.isFinite(count) ? String(count) : null;
}

function compareResultHtml(result) {
  const rows = (result && result.rows) || [];
  if (!rows.length) return `<p class="ho-hint">${esc(t("compare-need"))}</p>`;
  const text = (value, kind) => {
    const missing = missingText(value, kind);
    return missing ? { num: missing, unit: "" } : { num: prettyPhrase(String(value)), unit: "" };
  };
  const at = compareTable(rows, t("compare-at"), [
    [t("drop"), (row) => splitMeasure(row.drop, 2)],
    [t("elevation"), (row) => holdMeasure(row.elevation)],
    [t("windage"), (row) => holdMeasure(row.windage)],
    [t("clicks-heading"), (row) => {
      const elev = clickCount(row.elevation);
      const wind = clickCount(row.windage);
      if (elev == null && wind == null) return { num: missingText("", "click"), unit: "" };
      return { num: `${elev == null ? t("missing-not-set") : elev} / ${wind == null ? t("missing-not-set") : wind}`, unit: "" };
    }],
    [t("velocity"), (row) => splitMeasure(row.velocity, 0)],
    [t("energy"), (row) => splitMeasure(row.energy, 0)],
    [t("tof"), (row) => ({ num: Number(row.tof_s).toFixed(3), unit: "s" })],
    [t("mach"), (row) => ({ num: Number(row.mach).toFixed(2), unit: "" })],
    [t("stability"), (row) => (row.sg == null || row.sg === "" ? { num: t("missing-not-set"), unit: "" } : { num: Number(row.sg).toFixed(2), unit: "" })],
  ]);
  const gear = compareTable(rows, t("compare-equipment"), [
    [t("equip-rifle"), (row) => text(row.rifle)],
    [t("barrel"), (row) => splitMeasure(row.barrel, 1)],
    [t("twist"), (row) => (missingText(row.twist, "twist") ? { num: t("missing-no-twist"), unit: "" } : { num: prettyPhrase(row.twist), unit: row.twist_hand || "" })],
    [t("sight-height"), (row) => splitMeasure(row.sight_height, 2)],
    [t("rifle-weight"), (row) => splitMeasure(row.weight, 2)],
    [t("equip-scope"), (row) => text(row.scope, "scope")],
    [t("focal-plane"), (row) => text(row.focal_plane)],
    [t("click"), (row) => (missingText(row.click, "click") ? { num: t("missing-no-scope"), unit: "" } : splitMeasure(row.click))],
    [t("equip-bullet"), (row) => text(row.bullet)],
    [t("compare-drag"), (row) => text(row.drag)],
    [t("compare-bc"), (row) => text(row.bc)],
    [t("bullet-weight"), (row) => splitMeasure(row.bullet_weight, 2)],
    [t("equip-load"), (row) => text(row.load)],
    [t("mv"), (row) => splitMeasure(row.mv, 1)],
    [t("zero"), (row) => text(prettyPhrase(row.zero || ""))],
    [t("compare-truing"), (row) => text(row.truing)],
  ]);
  const warnings = rows.flatMap((row) => (row.warnings || []).map((item) => `${row.label}: ${item}`));
  const need = rows.length < 2 ? `<p class="ho-hint">${esc(t("compare-need"))}</p>` : "";
  const limit = compareError ? `<p class="error">${esc(compareError)}</p>` : "";
  return `${limit}${need}<div class="stack">${at}${gear}</div>${warnings.length ? `<p class="ho-hint">${esc(warnings.join(" "))}</p>` : ""}`;
}

function documentsHtml() {
  const docs = boot.equipment.documents || [];
  const current = docs.find((item) => item.id === docId) || {};
  const loads = (boot.equipment.loads || []).map((item) => `<option value="${esc(item.id)}" ${item.id === current.load_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const rifles = (boot.equipment.rifles || []).map((item) => `<option value="${esc(item.id)}" ${item.id === current.rifle_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const bullets = (boot.equipment.bullets || []).map((item) => `<option value="${esc(item.id)}" ${item.id === current.bullet_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const attached = current.filename ? `<p class="ho-hint">${esc(t("documents-attached"))}: ${esc(current.filename)}</p>` : "";
  const rail = `<h2>${esc(t("doc-add"))}</h2>
    <p class="ho-hint">${esc(t("documents-intro"))}</p>
    <input type="hidden" id="doc-id" value="${esc(current.id || "")}" />
    ${textInput("doc-title", t("documents-title"), current.title || "")}
    ${textInput("doc-edition", t("documents-edition"), current.edition || "")}
    ${textInput("doc-date", t("documents-date"), current.edition_date || "")}
    <div class="ho-field"><div class="head"><label for="doc-link">${esc(t("documents-link"))}</label></div><div class="ho-entry"><input id="doc-link" value="${esc(current.link_url || "")}" title="${esc(t("documents-link-tip"))}" autocomplete="off" /></div></div>
    <div class="ho-field"><div class="head"><label for="doc-file">${esc(t("documents-file"))}</label></div><input id="doc-file" type="file" accept="application/pdf,.pdf" title="${esc(t("documents-file-tip"))}" /></div>
    ${attached}
    ${hoSelect("doc-load", t("equip-load"), `<option value=""></option>${loads}`)}
    ${hoSelect("doc-rifle", t("equip-rifle"), `<option value=""></option>${rifles}`)}
    ${hoSelect("doc-bullet", t("equip-bullet"), `<option value=""></option>${bullets}`)}
    ${textInput("doc-notes", t("log-notes"), current.notes || "")}
    <div class="btnrow">
      <button type="button" class="ho-btn primary" id="doc-save">${esc(t("save"))}</button>
      <button type="button" class="ho-btn ghost" id="doc-new">${esc(t("new"))}</button>
      ${current.id ? `<button type="button" class="ho-btn" id="doc-open">${esc(t("documents-open"))}</button>` : ""}
      ${current.id ? `<button type="button" class="ho-btn" id="doc-archive">${esc(t("archive"))}</button>` : ""}
    </div>
    <p id="doc-msg" class="meta">${esc(docNotice)}</p>`;
  const rows = docs.map((item) => `<tr>
    <td class="text"><button type="button" class="ho-btn ghost" data-doc="${esc(item.id)}">${esc(item.title || item.label)}</button></td>
    <td class="text">${esc(item.edition || "")}</td>
    <td>${esc(item.edition_date || "")}</td>
  </tr>`).join("");
  const main = `<section class="ho-card flush"><div class="ho-table-wrap"><table class="ho-table"><thead><tr>
      <th class="text">${esc(t("documents-title"))}</th><th class="text">${esc(t("documents-edition"))}</th><th>${esc(t("documents-date"))}</th>
    </tr></thead><tbody>${rows || `<tr><td class="text" colspan="3">${esc(t("documents-none"))}</td></tr>`}</tbody></table></div></section>`;
  return frame({ rail, main, body: "with-list", railLabel: t("nav-documents") });
}

function bindDocuments() {
  document.querySelectorAll("[data-doc]").forEach((button) => button.addEventListener("click", () => {
    docId = button.dataset.doc;
    docNotice = "";
    render();
  }));
  const newer = document.getElementById("doc-new");
  if (newer) newer.addEventListener("click", () => { docId = ""; docNotice = ""; render(); });
  const save = document.getElementById("doc-save");
  if (save) save.addEventListener("click", saveDocument);
  const open = document.getElementById("doc-open");
  if (open) open.addEventListener("click", () => openDocument(docId));
  const archive = document.getElementById("doc-archive");
  if (archive) archive.addEventListener("click", archiveDocument);
  const date = document.getElementById("doc-date");
  if (date) date.title = t("documents-date-tip");
}

async function saveDocument() {
  const msg = document.getElementById("doc-msg");
  try {
    const body = {
      id: (document.getElementById("doc-id") || {}).value || "",
      title: (document.getElementById("doc-title") || {}).value || "",
      edition: (document.getElementById("doc-edition") || {}).value || "",
      edition_date: (document.getElementById("doc-date") || {}).value || "",
      link_url: (document.getElementById("doc-link") || {}).value || "",
      notes: (document.getElementById("doc-notes") || {}).value || "",
      load_id: (document.getElementById("doc-load") || {}).value || "",
      rifle_id: (document.getElementById("doc-rifle") || {}).value || "",
      bullet_id: (document.getElementById("doc-bullet") || {}).value || "",
    };
    const file = document.getElementById("doc-file") && document.getElementById("doc-file").files[0];
    if (file) body.file = { filename: file.name, base64: await fileBase64(file) };
    const saved = await call("save_document", { body });
    docId = saved.id;
    docNotice = t("saved");
    boot = await call("bootstrap");
    render();
  } catch (error) {
    if (msg) msg.textContent = String(error);
  }
}

function fileBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const text = String(reader.result || "");
      const comma = text.indexOf(",");
      resolve(comma >= 0 ? text.slice(comma + 1) : text);
    };
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

async function openDocument(id) {
  const msg = document.getElementById("doc-msg");
  try {
    const result = await call("open_document", { id });
    if (msg) msg.textContent = result.path || "";
  } catch (error) {
    const text = String(error);
    if (msg) msg.textContent = text.indexOf("not on this computer") >= 0 ? t("documents-missing") : text;
  }
}

async function archiveDocument() {
  if (!docId || !window.confirm(t("archive-confirm"))) return;
  try {
    await call("delete_record", { kind: "document", id: docId });
    docId = "";
    boot = await call("bootstrap");
    render();
  } catch (error) {
    const msg = document.getElementById("doc-msg");
    if (msg) msg.textContent = String(error);
  }
}

function logForm() {
  const record = currentRecord() || {};
  const loads = boot.equipment.loads || [];
  const loadOptions = loads.map((item) => `<option value="${esc(item.id)}" ${item.id === record.load_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const load = loads.find((item) => item.id === (record.load_id || (loads[0] && loads[0].id))) || null;
  const mv = load ? `${t("log-mv")}: ${load.mv_display || ""} ${load.mv_unit || ""}`.trim() : t("log-load-missing");
  const chronos = (boot.equipment.chronos || []).filter((item) => !load || item.load_id === load.id);
  const chosen = new Set(record.chrono_ids || []);
    const chronoBoxes = chronos.map((item) => chronoRow(item, chosen.has(item.id))).join("") || `<p class="hint">${esc(t("log-chronos-none"))}</p>`;
  const docs = boot.equipment.documents || [];
  const docChosen = new Set(record.document_ids || []);
  const docBoxes = docs.map((item) => `<label><input type="checkbox" class="log-doc" value="${esc(item.id)}" ${docChosen.has(item.id) ? "checked" : ""} /> ${esc(item.title || item.label)}</label>`).join("") || `<p class="hint">${esc(t("log-docs-none"))}</p>`;
  const notice = equipNotice;
  equipNotice = "";
  return `<p class="hint">${esc(t("log-intro"))}</p>
    <p class="hint">${esc(t("log-boundary"))}</p>
    <input type="hidden" id="record-id" value="${esc(record.id || "")}" />
    <div class="fields">
      ${textInput("label", t("label"), record.label)}
      <label class="field">${esc(t("equip-load"))}<select id="load_id">${loadOptions}</select></label>
      <p class="meta" id="log-mv">${esc(mv)}</p>
      ${textInput("powder", t("log-powder"), record.powder)}
      ${textInput("powder_lot", t("log-powder-lot"), record.powder_lot)}
      ${textInput("primer", t("log-primer"), record.primer)}
      ${textInput("primer_lot", t("log-primer-lot"), record.primer_lot)}
      ${textInput("case_brand", t("log-case"), record.case_brand)}
      ${textInput("case_lot", t("log-case-lot"), record.case_lot)}
      ${textInput("bullet_lot", t("log-bullet-lot"), record.bullet_lot)}
      ${recordField(record, "charge", t("log-charge"), "mass", units().mass)}
      ${recordField(record, "coal", t("log-coal"), "length", units().small)}
      ${textInput("notes", t("log-notes"), record.notes)}
      <p class="meta">${esc(t("log-chronos"))}</p>
      <div class="checks" id="log-chronos">${chronoBoxes}</div>
      <p class="meta">${esc(t("log-docs"))}</p>
      <div class="checks">${docBoxes}</div>
      <p class="meta">${esc(t("group-log"))}</p>
      <div class="row">${groupLinksForLoad(record.load_id)}</div>
    </div>
    <div class="row"><button type="button" id="equip-save">${esc(t("save"))}</button>
    ${record.id ? `<button type="button" id="equip-delete">${esc(t("archive"))}</button>` : ""}</div>
    <p id="equip-msg" class="meta">${esc(notice)}</p>`;
}

function chronoRow(item, checked) {
  const mean = Number.isFinite(Number(item.mean_mps)) ? `${formatLocal(Number(item.mean_mps), "velocity", units().velocity)} ${shownUnit(units().velocity)}` : "";
  return `<div class="row"><label><input type="checkbox" class="log-chrono" value="${esc(item.id)}" ${checked ? "checked" : ""} /> ${esc(item.label)} ${esc(mean)}</label>
    <button type="button" data-use-mv="${esc(item.id)}">${esc(t("chrono-set-mv"))}</button></div>`;
}

function bindLog() {
  const select = document.getElementById("load_id");
  if (!select) return;
  select.addEventListener("change", () => {
    const load = (boot.equipment.loads || []).find((item) => item.id === select.value);
    const node = document.getElementById("log-mv");
    if (node && load) node.textContent = `${t("log-mv")}: ${load.mv_display || ""} ${load.mv_unit || ""}`.trim();
    const box = document.getElementById("log-chronos");
    if (!box || !load) return;
    const chronos = (boot.equipment.chronos || []).filter((item) => item.load_id === load.id);
    box.innerHTML = chronos.map((item) => chronoRow(item, false)).join("") || `<p class="hint">${esc(t("log-chronos-none"))}</p>`;
    bindLogMv();
  });
  bindLogMv();
}

function bindLogMv() {
  document.querySelectorAll("[data-use-mv]").forEach((button) => {
    button.addEventListener("click", async (event) => {
      event.preventDefault();
      const chrono = (boot.equipment.chronos || []).find((item) => item.id === button.dataset.useMv);
      if (!chrono || !window.confirm(t("chrono-mv-confirm"))) return;
      try {
        await call("chrono_session", { body: { op: "set_mv", confirm: true, load_id: chrono.load_id, mean_mps: chrono.mean_mps, temperature_c: chrono.temperature_c } });
        boot = await call("bootstrap");
        equipNotice = t("saved");
        render();
      } catch (error) {
        const msg = document.getElementById("equip-msg");
        if (msg) msg.textContent = String(error);
      }
    });
  });
}

function archiveHtml() {
  const archived = boot.equipment.archived || [];
  const history = (boot.equipment.history && boot.equipment.history.rows) || [];
  const count = (boot.equipment.history && boot.equipment.history.count) || 0;
  const archivedRows = archived.map((item) => `<tr>
    <td>${esc(kindLabel(item.kind))}</td><td>${esc(item.label)}</td><td>${esc(item.deleted_at)}</td>
    <td><button type="button" data-restore-kind="${esc(item.kind)}" data-restore-id="${esc(item.id)}">${esc(t("archive-restore"))}</button></td>
  </tr>`).join("");
  const historyRows = history.map((item) => `<tr>
    <td>${esc(kindLabel(item.subject_kind))}</td><td>${esc(item.subject_label || "")}</td>
    <td>${esc(item.subject_revision)}</td><td>${esc(item.created_at)}</td>
    <td><button type="button" data-history="${esc(item.id)}">${esc(t("history-restore"))}</button></td>
  </tr>`).join("");
  const more = count > history.length ? `<p class="hint">${esc(t("history-more"))}</p>` : "";
  return `<p class="hint">${esc(t("archive-intro"))}</p>
    <h2>${esc(t("archive-title"))}</h2>
    ${archived.length ? `<div class="table-wrap"><table class="words"><thead><tr><th scope="col">${esc(t("archive-kind"))}</th><th scope="col">${esc(t("archive-name"))}</th><th scope="col">${esc(t("archive-when"))}</th><th scope="col">${esc(t("archive-restore"))}</th></tr></thead><tbody>${archivedRows}</tbody></table></div>` : `<p>${esc(t("archive-empty"))}</p>`}
    <h2>${esc(t("history-title"))}</h2>
    <p class="hint">${esc(t("history-intro"))}</p>
    ${history.length ? `<div class="table-wrap"><table class="words"><thead><tr><th scope="col">${esc(t("archive-kind"))}</th><th scope="col">${esc(t("archive-name"))}</th><th scope="col">${esc(t("history-revision"))}</th><th scope="col">${esc(t("history-when"))}</th><th scope="col">${esc(t("history-restore"))}</th></tr></thead><tbody>${historyRows}</tbody></table></div>${more}` : `<p>${esc(t("history-empty"))}</p>`}`;
}

function kindLabel(kind) {
  const keys = { rifle: "equip-rifle", scope: "equip-scope", bullet: "equip-bullet", load: "equip-load", profile: "equip-profile", chrono: "equip-chrono", reticle: "nav-reticle", target: "nav-targets", document: "nav-documents", load_log: "equip-log", atmosphere: "zero-air", history: "history-title", group: "nav-groups" };
  return t(keys[kind] || "archive");
}

function bindArchive() {
  document.querySelectorAll("[data-restore-kind]").forEach((button) => {
    button.addEventListener("click", async () => {
      try {
        await call("restore_record", { kind: button.dataset.restoreKind, id: button.dataset.restoreId });
        boot = await call("bootstrap");
        render();
      } catch (error) {
        window.alert(String(error));
      }
    });
  });
  document.querySelectorAll("[data-history]").forEach((button) => {
    button.addEventListener("click", async () => {
      if (!window.confirm(t("history-confirm"))) return;
      try {
        await call("restore_history", { id: button.dataset.history });
        boot = await call("bootstrap");
        render();
      } catch (error) {
        window.alert(String(error));
      }
    });
  });
}

function aboutHtml() {
  const main = `<div class="cols">
      <section class="ho-card stack">
        <header><h2>${esc(t("nav-about"))}</h2></header>
        <p class="ho-hint">${esc(t("about-intro"))}</p>
        <p class="big">${esc(boot.version || "")} <small>${esc(t("about-version"))}</small></p>
      </section>
      <section class="ho-card stack">
        <header><h2>${esc(t("about-mit-heading"))}</h2></header>
        <p>${esc(t("about-mit"))}</p>
        <pre class="licence">${esc(boot.licence || "")}</pre>
      </section>
      <section class="ho-card stack">
        <header><h2>${esc(t("about-weather-heading"))}</h2></header>
        <p>${esc(t("weather-attribution"))}</p>
        <p class="ho-hint">${esc(t("weather-noncommercial"))}</p>
        <p>${esc(t("about-personal"))}</p>
      </section>
      <section class="ho-card stack">
        <header><h2>${esc(t("about-drag-heading"))}</h2></header>
        <p>${esc(t("about-drag"))}</p>
      </section>
      <section class="ho-card stack">
        <header><h2>${esc(t("about-crates-heading"))}</h2></header>
        <p>${esc(t("about-crates"))}</p>
      </section>
      <section class="ho-card stack">
        <header><h2>${esc(t("about-limits-heading"))}</h2></header>
        <p>${esc(t("about-limits"))}</p>
        <p class="ho-hint">${esc(boot.spin_provenance || "")}</p>
        <p class="ho-hint">${esc(boot.ejecta_convention || "")}</p>
        <p class="ho-hint">${esc(t("about-keys"))}</p>
        <p class="ho-hint">${esc(t("about-package"))}</p>
      </section>
    </div>`;
  return frame({ body: "no-rail", main });
}

function freshGroup() {
  return {
    id: "",
    label: "Group",
    shotAt: "",
    distanceText: "100",
    distanceUnit: "",
    profileId: "",
    loadId: "",
    chronoId: "",
    targetId: "",
    ringing: "primary",
    notes: "",
    expectedShots: "5",
    measure: "centres",
    diameterText: "7.82",
    diameterUnit: "mm",
    knownText: "200",
    knownUnit: "mm",
    keepLocation: false,
    latitude: null,
    longitude: null,
    photo: null,
    image: null,
    mode: "four",
    corners: [],
    widthText: "200",
    heightText: "280",
    useCentre: true,
    poa: null,
    holes: [],
    activeHole: 0,
    tool: "corners",
    tab: "calibrate",
    zoom: 1,
    panX: 0,
    panY: 0,
    threshold: 90,
    analysis: null,
    notice: "",
    error: "",
    drag: null,
    view: null,
  };
}

function groupState() {
  if (!groupView) groupView = freshGroup();
  return groupView;
}

function groupFromRecord(record) {
  const state = freshGroup();
  const calibration = record.calibration || {};
  state.id = record.id || "";
  state.label = record.label || "Group";
  state.shotAt = record.shot_at || "";
  state.distanceText = Number.isFinite(Number(record.distance_m)) ? formatLocal(Number(record.distance_m), "length", units().length) : "100";
  state.distanceUnit = units().length;
  state.profileId = record.profile_id || "";
  state.loadId = record.load_id || "";
  state.chronoId = record.chrono_id || "";
  state.targetId = record.target_id || "";
  state.ringing = record.ringing === "alt" ? "alt" : "primary";
  state.notes = record.notes || "";
  state.expectedShots = String(record.expected_shots || 1);
  state.measure = record.measure === "edges" ? "edges" : "centres";
  if (Number.isFinite(Number(record.bullet_diameter_mm))) {
    state.diameterText = formatLocal(Number(record.bullet_diameter_mm) / 1000, "length", units().small);
    state.diameterUnit = units().small;
  }
  state.keepLocation = !!record.keep_location;
  state.latitude = record.latitude == null ? null : record.latitude;
  state.longitude = record.longitude == null ? null : record.longitude;
  state.photo = record.photo || null;
  state.mode = calibration.mode === "two" || calibration.mode === "two-point" ? "two" : "four";
  state.corners = (calibration.image_points || []).map((point) => ({ x: Number(point.x), y: Number(point.y) }));
  state.widthText = calibration.width_mm ? String(calibration.width_mm) : "200";
  state.heightText = calibration.height_mm ? String(calibration.height_mm) : "280";
  if (Number.isFinite(Number(calibration.known_mm))) {
    state.knownText = formatLocal(Number(calibration.known_mm) / 1000, "length", units().small);
    state.knownUnit = units().small;
  }
  state.useCentre = !!record.use_rectangle_centre;
  state.poa = pointObject(record.poa_image);
  state.holes = (record.holes || []).map((hole) => ({
    image: pointObject(hole.image) || { x: 0, y: 0 },
    count: Math.min(12, Math.max(1, Number(hole.count) || 1)),
  }));
  state.analysis = record.analysis || null;
  return state;
}

function pointObject(value) {
  if (!value) return null;
  if (Array.isArray(value) && value.length >= 2) return { x: Number(value[0]), y: Number(value[1]) };
  if (Number.isFinite(Number(value.x)) && Number.isFinite(Number(value.y))) return { x: Number(value.x), y: Number(value.y) };
  return null;
}

function openGroup(id) {
  const row = (boot.equipment.groups || []).find((item) => item.id === id);
  if (!row) return;
  groupView = groupFromRecord(row);
  page = "groups";
  render();
}

function groupsEquipHtml() {
  const record = currentRecord();
  if (!record) {
    return `<p class="hint">${esc(t("group-equip-empty"))}</p><button type="button" id="equip-new-group">${esc(t("group-new"))}</button>`;
  }
  return `<p>${esc(record.label)}</p>
    <p class="meta">${esc(record.distance_text || "")}</p>
    <div class="row">
      <button type="button" id="equip-open-group">${esc(t("group-open"))}</button>
      <button type="button" id="equip-new-group">${esc(t("group-new"))}</button>
      <button type="button" id="equip-delete">${esc(t("archive"))}</button>
    </div>`;
}

function groupLinksForLoad(loadId) {
  const linked = (boot.equipment.groups || []).filter((item) => loadId && item.load_id === loadId);
  if (!linked.length) return `<p class="hint">${esc(t("group-log-empty"))}</p>`;
  return linked.map((item) => `<button type="button" data-open-group="${esc(item.id)}">${esc(item.label)}</button>`).join("");
}

function groupsHtml() {
  const state = groupState();
  const rail = `<div class="ho-seg wrap" role="tablist" aria-label="${esc(t("group-mode"))}">${groupTabButton("calibrate")}${groupTabButton("holes")}${groupTabButton("context")}</div>
    <div class="stack">${groupAlwaysHtml()}
      ${state.tab === "holes" ? groupHolesHtml() : state.tab === "context" ? groupContextHtml() : groupCalibrateHtml()}
    </div>
    <p id="group-error" class="error">${esc(state.error || "")}</p>
    <p id="group-notice" class="meta">${esc(state.notice || "")}</p>`;
  const main = `<div class="photo">
      <div class="tools" role="toolbar" aria-label="${esc(t("group-mode"))}">${groupToolButtons()}</div>
      <div class="group-canvas-wrap"><canvas id="group-canvas" aria-label="${esc(t("group-photo"))}"></canvas></div>
      <div class="stack" id="group-results">${groupResultsInner()}</div>
    </div>`;
  return frame({ rail, main, body: "with-list", railLabel: t("nav-groups") });
}

function groupTabButton(id) {
  const selected = groupState().tab === id;
  return `<button type="button" role="tab" data-group-tab="${id}" aria-selected="${selected}">${esc(t("group-tab-" + id))}</button>`;
}

function groupToolButtons() {
  const state = groupState();
  const locked = !state.photo;
  const dis = locked ? " disabled" : "";
  const tool = (id) => `<button type="button" class="ho-btn" data-group-tool="${id}" aria-pressed="${state.tool === id}"${dis} title="${esc(t("group-tool-" + id))}">${esc(t("group-tool-" + id))}</button>`;
  return `${tool("corners")}${tool("aim")}${tool("hole")}${tool("move")}${tool("delete")}${tool("pan")}
    <button type="button" class="ho-btn ghost" id="group-zoom-in"${dis} title="${esc(t("group-zoom-in"))}">${esc(t("group-zoom-in"))}</button>
    <button type="button" class="ho-btn ghost" id="group-zoom-out"${dis} title="${esc(t("group-zoom-out"))}">${esc(t("group-zoom-out"))}</button>
    <button type="button" class="ho-btn ghost" id="group-zoom-fit"${dis} title="${esc(t("group-zoom-fit"))}">${esc(t("group-zoom-fit"))}</button>`;
}

function groupAlwaysHtml() {
  const state = groupState();
  const distanceUnit = state.distanceUnit || units().length;
  const ready = state.analysis && state.analysis.can_save;
  return `<div class="cluster">
      ${qtyField("group-distance", t("group-distance"), state.distanceText, distanceUnit, "length")}
      <label class="field">${esc(t("group-expected"))}<input id="group-expected" inputmode="numeric" value="${esc(state.expectedShots)}" /></label>
      <button type="button" id="group-save" ${ready ? "" : "disabled"}>${esc(t("group-save"))}</button>
    </div>`;
}

function groupCalibrateHtml() {
  const state = groupState();
  const knownUnit = state.knownUnit || "mm";
  const target = libraryTarget(state.targetId);
  const sheet = target && target.official_bounds && target.width_cm && target.height_cm
    ? `<button type="button" id="group-sheet">${esc(t("group-sheet"))}</button>` : "";
  return `<div class="compact">
    <div class="cluster">
      <label class="field">${esc(t("group-saved"))}<select id="group-saved">${groupSavedOptions()}</select></label>
      <label class="field">${esc(t("group-photo"))}<input id="group-file" type="file" accept="image/jpeg,image/png,.jpg,.jpeg,.png,.heic,.heif" /></label>
      <label title="${esc(t("group-photo-keep-tip"))}"><input id="group-keep" type="checkbox" ${state.keepLocation ? "checked" : ""} /> ${esc(t("group-photo-keep"))}</label>
    </div>
    <div class="cluster">
      <label class="field">${esc(t("group-mode"))}
        <select id="group-mode">
          <option value="four" ${state.mode === "four" ? "selected" : ""}>${esc(t("group-mode-four"))}</option>
          <option value="two" ${state.mode === "two" ? "selected" : ""}>${esc(t("group-mode-two"))}</option>
        </select>
      </label>
      ${state.mode === "two" ? qtyField("group-known", t("group-known"), state.knownText, knownUnit, "length") : ""}
      ${state.mode === "four" ? `<label class="field">${esc(t("group-width"))}<input id="group-width" value="${esc(state.widthText)}" /></label>
        <label class="field">${esc(t("group-height"))}<input id="group-height" value="${esc(state.heightText)}" /></label>` : ""}
    </div>
    ${state.mode === "four" ? `<label title="${esc(t("group-centre"))}"><input id="group-centre" type="checkbox" ${state.useCentre ? "checked" : ""} /> ${esc(t("group-centre"))}</label>` : ""}
    <p class="hint">${esc(t(state.mode === "two" ? "group-two-hint" : "group-corners-hint"))}</p>
    <div class="cluster">
      ${sheet}
      <button type="button" id="group-clear-corners">${esc(t("group-clear-corners"))}</button>
      <label class="field">${esc(t("group-threshold"))}<input id="group-threshold" type="range" min="0" max="255" value="${esc(String(state.threshold))}" /></label>
      <button type="button" id="group-detect">${esc(t("group-detect"))}</button>
    </div>
    <p class="hint">${esc(t("group-canvas-empty"))}</p>
  </div>`;
}

function groupHolesHtml() {
  const state = groupState();
  const diameterUnit = state.diameterUnit || units().small;
  const hole = state.holes[state.activeHole] || null;
  return `<div class="compact">
    <div class="cluster">
      <label class="field">${esc(t("group-measure"))}
        <select id="group-measure">
          <option value="centres" ${state.measure !== "edges" ? "selected" : ""}>${esc(t("group-centres"))}</option>
          <option value="edges" ${state.measure === "edges" ? "selected" : ""}>${esc(t("group-edges"))}</option>
        </select>
      </label>
      ${qtyField("group-diameter", t("group-diameter"), state.diameterText, diameterUnit, "length")}
      <label class="field">${esc(t("group-count"))}<input id="group-count" inputmode="numeric" value="${esc(hole ? String(hole.count) : "1")}" /></label>
    </div>
    <p class="meta">${esc(t("group-shots"))}: ${esc(String(groupShotCount()))}</p>
    <button type="button" id="group-clear-holes">${esc(t("group-clear-holes"))}</button>
  </div>`;
}

function groupContextHtml() {
  const state = groupState();
  return `<div class="compact">
    <div class="cluster">
      <label class="field">${esc(t("label"))}<input id="group-label" value="${esc(state.label)}" /></label>
      <label class="field">${esc(t("group-when"))}<input id="group-when" type="datetime-local" value="${esc(state.shotAt)}" /></label>
    </div>
    <div class="cluster">
      <label class="field">${esc(t("equip-profile"))}<select id="group-profile">${idOptions(boot.equipment.profiles, state.profileId)}</select></label>
      <label class="field">${esc(t("equip-load"))}<select id="group-load">${idOptions(boot.equipment.loads, state.loadId)}</select></label>
      <label class="field">${esc(t("equip-chrono"))}<select id="group-chrono">${idOptions(boot.equipment.chronos, state.chronoId)}</select></label>
    </div>
    <div class="cluster">
      <label class="field">${esc(t("group-target"))}<select id="group-target">${groupTargetOptions(state.targetId)}</select></label>
      <label class="field">${esc(t("ringing"))}
        <select id="group-ringing">
          <option value="primary" ${state.ringing !== "alt" ? "selected" : ""}>${esc(t("ringing-primary"))}</option>
          <option value="alt" ${state.ringing === "alt" ? "selected" : ""}>${esc(t("ringing-alt"))}</option>
        </select>
      </label>
    </div>
    <label class="field">${esc(t("log-notes"))}<textarea id="group-notes">${esc(state.notes)}</textarea></label>
    <div class="row">
      <button type="button" id="group-truing">${esc(t("group-truing"))}</button>
      <button type="button" id="group-impact">${esc(t("group-impact"))}</button>
      <button type="button" id="group-new">${esc(t("group-new"))}</button>
    </div>
  </div>`;
}

function idOptions(items, selected) {
  const head = `<option value="">${esc(t("group-blank"))}</option>`;
  const rest = (items || []).map((item) => `<option value="${esc(item.id)}" ${item.id === selected ? "selected" : ""}>${esc(item.label || item.name || "")}</option>`).join("");
  return head + rest;
}

function groupSavedOptions() {
  const state = groupState();
  const head = `<option value="">${esc(t("group-new"))}</option>`;
  const rest = (boot.equipment.groups || []).map((item) => `<option value="${esc(item.id)}" ${item.id === state.id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  return head + rest;
}

function groupTargetOptions(selected) {
  const library = (boot.targets && boot.targets.library) || [];
  const saved = (boot.equipment && boot.equipment.targets) || [];
  const rank = { sjf: 0, svsf: 1, us: 2, generic: 3 };
  const seen = new Set();
  const items = library.concat(saved).filter((item) => {
    if (!item || seen.has(item.id)) return false;
    seen.add(item.id);
    return true;
  });
  items.sort((left, right) => (rank[left.family] ?? 4) - (rank[right.family] ?? 4) || String(left.name || "").localeCompare(String(right.name || "")));
  const head = `<option value="">${esc(t("group-none"))}</option>`;
  return head + items.map((item) => `<option value="${esc(item.id)}" ${item.id === selected ? "selected" : ""}>${esc(item.name)}</option>`).join("");
}

function groupResultsInner() {
  const analysis = groupState().analysis;
  const stats = analysis && analysis.stats;
  if (!stats) return `<p class="ho-hint">${esc(groupBlocked())}</p>`;
  const small = units().small;
  const linear = (mm) => `${formatLocal(Number(mm) / 1000, "length", small)} ${small}`;
  const angle = (mrad, moa) => (Number.isFinite(Number(mrad)) ? `${Number(mrad).toFixed(2)} mrad · ${Number(moa).toFixed(2)} MOA` : t("missing-not-set"));
  const score = scoreText(analysis.score);
  const warnings = (analysis.warnings || []).map((code) => t("group-warn-" + code)).filter((item) => item).join(" · ");
  const notes = analysis.score && analysis.score.notes ? analysis.score.notes.map((code) => t("group-" + code)).join(" · ") : "";
  const kv = (label, value) => `<dt>${esc(label)}</dt><dd>${esc(value)}</dd><span class="ho-unit"></span>`;
  return `<h3>${esc(t("group-offset"))}</h3>
    <p class="big">${esc(stats.horizontal || t("missing-not-set"))} <small>${esc(linear(stats.reported_offset_x_mm))} · ${esc(angle(stats.offset_x_mrad, stats.offset_x_moa))}</small></p>
    <p class="big">${esc(stats.vertical || t("missing-not-set"))} <small>${esc(linear(stats.reported_offset_y_mm))} · ${esc(angle(stats.offset_y_mrad, stats.offset_y_moa))}</small></p>
    <dl class="ho-kv">
      ${kv(t("group-shots"), String(analysis.shot_count || stats.n))}
      ${kv(t("group-es"), `${linear(stats.reported_extreme_spread_mm)} · ${angle(stats.es_mrad, stats.es_moa)}`)}
      ${kv(t("group-mr"), linear(stats.reported_mean_radius_mm))}
      ${kv(t("group-score"), score)}
      ${stats.cep_mm == null ? "" : kv(t("group-cep"), linear(stats.cep_mm))}
      ${kv(t("group-sd"), linear(stats.radial_sd_mm))}
    </dl>
    ${warnings ? `<p class="ho-hint">${esc(warnings)}</p>` : ""}
    ${notes ? `<p class="ho-hint">${esc(notes)}</p>` : ""}`;
}

function scoreText(score) {
  if (!score || score.available === false) {
    if (score && score.reason) return t("group-" + score.reason);
    return t("group-score-none");
  }
  if (score.total == null) return t("group-rings-without-points");
  const shots = (score.shots || []).map((shot) => shot.miss ? t("group-score-none") : (shot.label || "")).join(" ");
  const possible = score.possible == null ? "" : ` / ${score.possible}`;
  return `${score.total}${possible}${shots ? " · " + shots : ""}`;
}

function groupBlocked() {
  const analysis = groupState().analysis;
  if (!analysis) return t("group-corners-hint");
  if (analysis.reason) return t("group-reason-" + analysis.reason);
  if (!analysis.can_save) {
    const distance = groupState().distanceText;
    if (!distance) return t("group-reason-distance");
    return t("group-reason-count");
  }
  return t("group-ready");
}

function groupShotCount() {
  return groupState().holes.reduce((sum, hole) => sum + (Number(hole.count) || 1), 0);
}

function rememberGroupFields() {
  const state = groupState();
  const read = (id) => document.getElementById(id);
  const distance = read("group-distance");
  if (distance) {
    state.distanceText = distance.value;
    state.distanceUnit = distance.dataset.unit;
  }
  const diameter = read("group-diameter");
  if (diameter) {
    state.diameterText = diameter.value;
    state.diameterUnit = diameter.dataset.unit;
  }
  const known = read("group-known");
  if (known) {
    state.knownText = known.value;
    state.knownUnit = known.dataset.unit;
  }
  const text = (id, key) => {
    const node = read(id);
    if (node) state[key] = node.value;
  };
  text("group-expected", "expectedShots");
  text("group-width", "widthText");
  text("group-height", "heightText");
  text("group-label", "label");
  text("group-when", "shotAt");
  text("group-notes", "notes");
  const mode = read("group-mode");
  if (mode && mode.value !== state.mode) {
    state.mode = mode.value;
    state.corners = [];
  }
  const measure = read("group-measure");
  if (measure) state.measure = measure.value;
  const keep = read("group-keep");
  if (keep) state.keepLocation = keep.checked;
  const centre = read("group-centre");
  if (centre) state.useCentre = centre.checked;
  const threshold = read("group-threshold");
  if (threshold) state.threshold = Number(threshold.value);
  const count = read("group-count");
  if (count && state.holes[state.activeHole]) {
    state.holes[state.activeHole].count = Math.min(12, Math.max(1, Number(count.value) || 1));
  }
  ["group-profile", "group-load", "group-chrono", "group-target", "group-ringing"].forEach((id) => {
    const node = read(id);
    if (!node) return;
    const key = { "group-profile": "profileId", "group-load": "loadId", "group-chrono": "chronoId", "group-target": "targetId", "group-ringing": "ringing" }[id];
    state[key] = node.value;
  });
}

function mmNumber(text) {
  const value = Number(String(text == null ? "" : text).replace(",", "."));
  return Number.isFinite(value) ? value : 0;
}

function groupCalibration() {
  const state = groupState();
  const points = state.corners.map((point) => ({ x: point.x, y: point.y }));
  if (state.mode === "two") {
    const known = parseLocal(state.knownText, "length", state.knownUnit || "mm");
    return { mode: "two", image_points: points, known_mm: known ? known.si * 1000 : 0 };
  }
  return {
    mode: "four",
    image_points: points,
    width_mm: mmNumber(state.widthText),
    height_mm: mmNumber(state.heightText),
  };
}

function groupBody() {
  const state = groupState();
  const distanceUnit = state.distanceUnit || units().length;
  const diameterUnit = state.diameterUnit || units().small;
  const body = {
    label: state.label,
    shot_at: state.shotAt,
    distance: { text: state.distanceText, unit: distanceUnit },
    distance_text: `${state.distanceText} ${distanceUnit}`.trim(),
    bullet_diameter: { text: state.diameterText, unit: diameterUnit },
    bullet_diameter_text: `${state.diameterText} ${diameterUnit}`.trim(),
    expected_shots: Number(state.expectedShots) || 0,
    measure: state.measure,
    ringing: state.ringing,
    use_rectangle_centre: state.mode === "four" && state.useCentre,
    poa_image: state.poa,
    calibration: groupCalibration(),
    holes: state.holes.map((hole) => ({ image: hole.image, count: hole.count || 1 })),
    photo: state.photo,
    profile_id: state.profileId,
    load_id: state.loadId,
    chrono_id: state.chronoId,
    target_id: state.targetId,
    notes: state.notes,
    keep_location: state.keepLocation,
    threshold: Number(state.threshold) || 90,
  };
  if (state.id) body.id = state.id;
  if (state.keepLocation && state.latitude != null) body.latitude = state.latitude;
  if (state.keepLocation && state.longitude != null) body.longitude = state.longitude;
  return body;
}

function groupError(error) {
  const text = String(error || "");
  const keys = {
    "photo-heic": "group-photo-heic",
    "photo-type": "group-photo-type",
    "photo-empty": "group-photo-empty",
    "photo-large": "group-photo-large",
    "photo-decode": "group-photo-decode",
    "photo-missing": "group-photo-missing",
  };
  return t(keys[text] || text);
}

function scheduleGroup(delay) {
  window.clearTimeout(groupTimer);
  groupTimer = window.setTimeout(runGroupAnalysis, delay);
}

async function runGroupAnalysis() {
  if (page !== "groups") return;
  rememberGroupFields();
  try {
    const analysis = await call("analyze_group", { body: groupBody() });
    groupState().analysis = analysis;
    groupState().error = "";
    const host = document.getElementById("group-results");
    if (host) host.innerHTML = groupResultsInner();
    const save = document.getElementById("group-save");
    if (save) save.disabled = !(analysis && analysis.can_save);
    paintGroup();
  } catch (error) {
    groupState().error = groupError(error);
    const node = document.getElementById("group-error");
    if (node) node.textContent = groupState().error;
  }
}

function bindGroups() {
  rememberGroupFields();
  document.querySelectorAll("[data-group-tab]").forEach((button) => button.addEventListener("click", () => {
    rememberGroupFields();
    groupState().tab = button.dataset.groupTab;
    groupState().notice = "";
    render();
  }));
  document.querySelectorAll("[data-group-tool]").forEach((button) => button.addEventListener("click", () => {
    groupState().tool = button.dataset.groupTool;
    document.querySelectorAll("[data-group-tool]").forEach((item) => item.setAttribute("aria-pressed", item === button ? "true" : "false"));
  }));
  const file = document.getElementById("group-file");
  if (file) file.addEventListener("change", () => {
    const chosen = file.files && file.files[0];
    if (chosen) importGroupFile(chosen);
  });
  const saved = document.getElementById("group-saved");
  if (saved) saved.addEventListener("change", () => {
    if (!saved.value) {
      groupView = freshGroup();
      render();
      return;
    }
    openGroup(saved.value);
  });
  const mode = document.getElementById("group-mode");
  if (mode) mode.addEventListener("change", () => {
    rememberGroupFields();
    render();
  });
  ["group-width", "group-height", "group-expected", "group-count", "group-label", "group-notes", "group-when"].forEach((id) => {
    const node = document.getElementById(id);
    if (node) node.addEventListener("input", () => scheduleGroup(160));
  });
  ["group-profile", "group-load", "group-chrono", "group-target", "group-ringing", "group-measure"].forEach((id) => {
    const node = document.getElementById(id);
    if (node) node.addEventListener("change", () => {
      rememberGroupFields();
      if (id === "group-target") render();
      else scheduleGroup(0);
    });
  });
  const centre = document.getElementById("group-centre");
  if (centre) centre.addEventListener("change", () => scheduleGroup(0));
  const keep = document.getElementById("group-keep");
  if (keep) keep.addEventListener("change", () => { groupState().keepLocation = keep.checked; });
  const sheet = document.getElementById("group-sheet");
  if (sheet) sheet.addEventListener("click", () => {
    const target = libraryTarget(groupState().targetId);
    if (!target || !target.official_bounds || !target.width_cm || !target.height_cm) return;
    groupState().widthText = String(Math.round(target.width_cm * 10));
    groupState().heightText = String(Math.round(target.height_cm * 10));
    render();
  });
  const clearCorners = document.getElementById("group-clear-corners");
  if (clearCorners) clearCorners.addEventListener("click", () => {
    groupState().corners = [];
    scheduleGroup(0);
    paintGroup();
  });
  const clearHoles = document.getElementById("group-clear-holes");
  if (clearHoles) clearHoles.addEventListener("click", () => {
    groupState().holes = [];
    scheduleGroup(0);
    paintGroup();
  });
  const detect = document.getElementById("group-detect");
  if (detect) detect.addEventListener("click", detectGroupHoles);
  const save = document.getElementById("group-save");
  if (save) save.addEventListener("click", saveGroup);
  const truing = document.getElementById("group-truing");
  if (truing) truing.addEventListener("click", sendGroupToTruing);
  const impact = document.getElementById("group-impact");
  if (impact) impact.addEventListener("click", showGroupOnImpact);
  const newer = document.getElementById("group-new");
  if (newer) newer.addEventListener("click", () => { groupView = freshGroup(); render(); });
  const zoomIn = document.getElementById("group-zoom-in");
  const zoomOut = document.getElementById("group-zoom-out");
  const zoomFit = document.getElementById("group-zoom-fit");
  if (zoomIn) zoomIn.addEventListener("click", () => { groupState().zoom = Math.min(12, groupState().zoom * 1.25); paintGroup(); });
  if (zoomOut) zoomOut.addEventListener("click", () => { groupState().zoom = Math.max(0.25, groupState().zoom / 1.25); paintGroup(); });
  if (zoomFit) zoomFit.addEventListener("click", () => { groupState().zoom = 1; groupState().panX = 0; groupState().panY = 0; paintGroup(); });
  const canvas = document.getElementById("group-canvas");
  if (canvas) bindGroupCanvas(canvas);
  const hash = groupState().photo && groupState().photo.sha256;
  if (hash) {
    ensureGroupImage(hash).then(() => paintGroup()).catch((error) => {
      groupState().error = groupError(error);
      const node = document.getElementById("group-error");
      if (node) node.textContent = groupState().error;
    });
  } else {
    requestAnimationFrame(() => paintGroup());
  }
  scheduleGroup(0);
}

function bindGroupCanvas(canvas) {
  if (groupObserver) groupObserver.disconnect();
  groupObserver = new ResizeObserver(() => paintGroup());
  groupObserver.observe(canvas.parentElement || canvas);
  canvas.addEventListener("pointerdown", (event) => {
    const state = groupState();
    const image = eventToImage(event);
    state.drag = { x: event.clientX, y: event.clientY, marker: nearestMarker(image) };
    if (canvas.setPointerCapture) canvas.setPointerCapture(event.pointerId);
    if (state.tool === "corners") {
      const limit = state.mode === "two" ? 2 : 4;
      if (state.corners.length < limit) state.corners.push(image);
    } else if (state.tool === "hole") {
      state.holes.push({ image, count: 1 });
      state.activeHole = state.holes.length - 1;
    } else if (state.tool === "aim") {
      state.poa = image;
      state.useCentre = false;
      const box = document.getElementById("group-centre");
      if (box) box.checked = false;
    } else if (state.tool === "delete" && state.drag.marker) {
      removeMarker(state.drag.marker);
    }
    if (state.tool !== "pan" && state.tool !== "move") scheduleGroup(40);
    paintGroup();
  });
  canvas.addEventListener("pointermove", (event) => {
    const state = groupState();
    if (!state.drag) return;
    if (state.tool === "pan") {
      state.panX += event.clientX - state.drag.x;
      state.panY += event.clientY - state.drag.y;
      state.drag.x = event.clientX;
      state.drag.y = event.clientY;
      paintGroup();
      return;
    }
    if (state.tool === "move" && state.drag.marker) {
      moveMarker(state.drag.marker, eventToImage(event));
      paintGroup();
    }
  });
  canvas.addEventListener("pointerup", () => {
    const state = groupState();
    if (state.drag && state.tool === "move") scheduleGroup(30);
    state.drag = null;
  });
  canvas.addEventListener("wheel", (event) => {
    event.preventDefault();
    const state = groupState();
    const factor = event.deltaY > 0 ? 0.9 : 1.1;
    state.zoom = Math.min(12, Math.max(0.25, (state.zoom || 1) * factor));
    paintGroup();
  }, { passive: false });
}

function removeMarker(marker) {
  const state = groupState();
  if (marker.kind === "corner") state.corners.splice(marker.index, 1);
  if (marker.kind === "hole") state.holes.splice(marker.index, 1);
  if (marker.kind === "aim") state.poa = null;
}

function moveMarker(marker, image) {
  const state = groupState();
  if (marker.kind === "corner" && state.corners[marker.index]) state.corners[marker.index] = image;
  if (marker.kind === "hole" && state.holes[marker.index]) state.holes[marker.index].image = image;
  if (marker.kind === "aim") state.poa = image;
}

async function importGroupFile(file) {
  rememberGroupFields();
  try {
    const bytesBase64 = await fileToBase64(file);
    const photo = await call("import_group_photo", {
      bytesBase64,
      keepLocation: groupState().keepLocation,
      filename: file.name || "photo.png",
    });
    const state = groupState();
    state.photo = { sha256: photo.sha256, media: photo.media, width: photo.width, height: photo.height, filename: photo.filename };
    state.latitude = photo.latitude == null ? null : photo.latitude;
    state.longitude = photo.longitude == null ? null : photo.longitude;
    state.corners = [];
    state.holes = [];
    state.poa = null;
    state.analysis = null;
    state.error = "";
    state.image = null;
    await ensureGroupImage(photo.sha256);
    render();
  } catch (error) {
    groupState().error = groupError(error);
    const node = document.getElementById("group-error");
    if (node) node.textContent = groupState().error;
  }
}

function fileToBase64(file) {
  return file.arrayBuffer().then((buffer) => {
    const bytes = new Uint8Array(buffer);
    let binary = "";
    const step = 0x8000;
    for (let index = 0; index < bytes.length; index += step) {
      binary += String.fromCharCode(...bytes.subarray(index, index + step));
    }
    return btoa(binary);
  });
}

async function ensureGroupImage(hash) {
  const state = groupState();
  if (state.image && state.image.dataset && state.image.dataset.hash === hash && state.image.complete) return;
  const payload = await call("group_photo", { hash });
  const img = new Image();
  img.dataset.hash = hash;
  await new Promise((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error("photo-decode"));
    img.src = `data:image/png;base64,${payload.base64}`;
  });
  if (groupState().photo && groupState().photo.sha256 === hash) groupState().image = img;
}

async function detectGroupHoles() {
  rememberGroupFields();
  if (groupState().holes.length && !window.confirm(t("group-detect-replace"))) return;
  try {
    const found = await call("detect_holes", { body: groupBody() });
    groupState().holes = (found.holes || []).map((point) => ({ image: { x: point.x, y: point.y }, count: 1 }));
    groupState().activeHole = 0;
    const warnings = (found.warnings || []).map((code) => t("group-warn-" + code)).join(" · ");
    groupState().notice = warnings;
    const note = document.getElementById("group-notice");
    if (note) note.textContent = warnings;
    scheduleGroup(0);
    paintGroup();
  } catch (error) {
    groupState().error = groupError(error);
    const node = document.getElementById("group-error");
    if (node) node.textContent = groupState().error;
  }
}

async function saveGroup() {
  rememberGroupFields();
  try {
    const saved = await call("save_group", { body: groupBody() });
    groupView = groupFromRecord(saved);
    groupState().notice = t("saved");
    boot = await call("bootstrap");
    render();
  } catch (error) {
    groupState().error = groupError(error);
    const node = document.getElementById("group-error");
    if (node) node.textContent = groupState().error;
  }
}

function sendGroupToTruing() {
  rememberGroupFields();
  const stats = groupState().analysis && groupState().analysis.stats;
  if (!stats) return;
  const unit = groupState().distanceUnit || units().length;
  truingForm.range = `${groupState().distanceText} ${unit}`.trim();
  truingForm.elevation = Number.isFinite(Number(stats.correction_elevation_mrad)) ? `${Number(stats.correction_elevation_mrad).toFixed(3)} mrad` : "";
  truingForm.shots = String(groupState().analysis.shot_count || "");
  truingForm.group = Number.isFinite(Number(stats.reported_extreme_spread_mm)) ? `${Number(stats.reported_extreme_spread_mm).toFixed(1)} mm` : "";
  page = "truing";
  render();
}

function showGroupOnImpact() {
  if (!groupState().id) {
    groupState().error = t("group-impact-first");
    const node = document.getElementById("group-error");
    if (node) node.textContent = groupState().error;
    return;
  }
  impactGroupId = groupState().id;
  groupImpactPending = true;
  page = "charts";
  render();
}

function cssColor(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "currentColor";
}

function paintGroup() {
  const canvas = document.getElementById("group-canvas");
  const state = groupState();
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  if (rect.width < 2 || rect.height < 2) return;
  const dpr = window.devicePixelRatio || 1;
  const pixelWidth = Math.max(1, Math.floor(rect.width * dpr));
  const pixelHeight = Math.max(1, Math.floor(rect.height * dpr));
  if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
    canvas.width = pixelWidth;
    canvas.height = pixelHeight;
  }
  const ctx = canvas.getContext("2d");
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = cssColor("--surface");
  ctx.fillRect(0, 0, rect.width, rect.height);
  const img = state.image;
  if (!img || !img.complete || !img.naturalWidth) {
    ctx.fillStyle = cssColor("--text-secondary");
    ctx.font = "14px sans-serif";
    ctx.fillText(t("group-canvas-wait"), 12, 24);
    return;
  }
  const fit = Math.min(rect.width / img.naturalWidth, rect.height / img.naturalHeight);
  const scale = fit * (state.zoom || 1);
  const ox = (rect.width - img.naturalWidth * scale) / 2 + (state.panX || 0);
  const oy = (rect.height - img.naturalHeight * scale) / 2 + (state.panY || 0);
  state.view = { scale, ox, oy };
  ctx.drawImage(img, ox, oy, img.naturalWidth * scale, img.naturalHeight * scale);
  const accent = cssColor("--accent");
  const text = cssColor("--text");
  const focus = cssColor("--focus");
  const warning = cssColor("--warning");
  state.corners.forEach((point, index) => markRing(ctx, imageToScreen(point, state.view), 8, accent, text, String(index + 1)));
  const aim = groupAimImage();
  if (aim) markCross(ctx, imageToScreen(aim, state.view), 9, focus);
  state.holes.forEach((hole) => markRing(ctx, imageToScreen(hole.image, state.view), 6, warning, text, hole.count > 1 ? String(hole.count) : ""));
}

function groupAimImage() {
  const state = groupState();
  if (state.mode === "four" && state.useCentre) return pointObject(state.analysis && state.analysis.poa_image);
  return state.poa;
}

function imageToScreen(point, view) {
  return { x: view.ox + point.x * view.scale, y: view.oy + point.y * view.scale };
}

function eventToImage(event) {
  const canvas = document.getElementById("group-canvas");
  const view = groupState().view;
  const rect = canvas.getBoundingClientRect();
  const x = event.clientX - rect.left;
  const y = event.clientY - rect.top;
  if (!view || !view.scale) return { x, y };
  return { x: (x - view.ox) / view.scale, y: (y - view.oy) / view.scale };
}

function nearestMarker(image) {
  const state = groupState();
  const view = state.view;
  if (!view) return null;
  let best = null;
  let bestPx = 16;
  const click = imageToScreen(image, view);
  const consider = (kind, index, point) => {
    if (!point) return;
    const at = imageToScreen(point, view);
    const dist = Math.hypot(at.x - click.x, at.y - click.y);
    if (dist < bestPx) {
      bestPx = dist;
      best = { kind, index };
    }
  };
  state.corners.forEach((point, index) => consider("corner", index, point));
  state.holes.forEach((hole, index) => consider("hole", index, hole.image));
  if (!(state.mode === "four" && state.useCentre)) consider("aim", 0, state.poa);
  return best;
}

function markRing(ctx, at, radius, stroke, text, label) {
  ctx.beginPath();
  ctx.arc(at.x, at.y, radius, 0, Math.PI * 2);
  ctx.strokeStyle = stroke;
  ctx.lineWidth = 2;
  ctx.stroke();
  if (label) {
    ctx.fillStyle = text;
    ctx.font = "12px sans-serif";
    ctx.fillText(label, at.x + radius + 2, at.y - 2);
  }
}

function markCross(ctx, at, size, stroke) {
  ctx.strokeStyle = stroke;
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(at.x - size, at.y);
  ctx.lineTo(at.x + size, at.y);
  ctx.moveTo(at.x, at.y - size);
  ctx.lineTo(at.x, at.y + size);
  ctx.stroke();
}

listenTheme();
refresh().catch((error) => {
  document.getElementById("app").textContent = String(error);
});
