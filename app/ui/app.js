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
let paletteOpen = false;
let unitsOpen = false;
let equipKind = "rifle";
let selectedId = "";
let restoreDoc = null;
let restorePreview = null;
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
let pbrSpan = "full";
let pbrWindow = null;
let cardConditions = "";
let cardBrackets = "2 mps, 4 mps, 6 mps";
let chartKinds = { drop: true, wind: true, velocity: false, energy: false, tof: false, mach: false };
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

function esc(value) {
  return String(value ?? "").replace(/[&<>"']/g, (ch) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[ch]);
}

async function call(command, args) {
  if (!window.__TAURI__ || !window.__TAURI__.core) {
    throw new Error("This window is not running inside Holdover.");
  }
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
  (theme.chart || []).forEach((color, index) => root.style.setProperty(`--chart-${index}`, color));
  root.style.colorScheme = theme.mode === "light" ? "light" : "dark";
  document.body.classList.toggle("large-text", !!(boot && boot.settings.large_text));
  document.body.classList.toggle("reduced", !!(boot && boot.settings.reduced_motion));
  document.body.classList.toggle("field", fieldMode);
}

async function refresh() {
  boot = await call("bootstrap");
  if (!cardPageTouched && boot.paper) cardPageId = boot.paper;
  applyTheme();
  render();
}

function render() {
  window.clearInterval(weatherTimer);
  const app = document.getElementById("app");
  if (!boot.settings.onboarded) {
    app.innerHTML = onboardHtml();
    bindOnboard();
    return;
  }
  app.innerHTML = `${shellHtml()}<main class="page" id="page" tabindex="-1">${pageHtml()}</main><footer class="footer"><p class="disclaimer">${esc(boot.disclaimer)}</p><button type="button" id="open-about">${esc(t("nav-about"))}</button></footer>${overlays()}`;
  bindShell();
  bindPage();
  if (page === "solve") scheduleSolve(0);
  if (page === "compare") scheduleCompare(0);
}

function shellHtml() {
  const preset = boot.settings.preset;
  return `<div class="shell"><a class="skip" href="#page">${esc(t("skip"))}</a><header class="top">
    <span class="brand">${esc(t("app-name"))}</span>
    <nav aria-label="${esc(t("nav-label"))}">
      ${nav("solve")}${nav("card")}${nav("charts")}${nav("reticle")}${nav("targets")}${nav("pbr")}${nav("truing")}${nav("equipment")}${nav("compare")}${nav("documents")}${nav("settings")}
    </nav>
    <div class="presets">
      ${presetButton("metric")}${presetButton("us")}${presetButton("custom")}
      <button type="button" id="units-open" aria-expanded="${unitsOpen}">${esc(t("units-button"))}</button>
    </div>
  </header>`;
  function nav(id) {
    const keys = { solve: "nav-solve", card: "nav-card", charts: "nav-charts", reticle: "nav-reticle", targets: "nav-targets", pbr: "nav-pbr", truing: "nav-truing", equipment: "nav-equipment", compare: "nav-compare", documents: "nav-documents", settings: "nav-settings" };
    const key = keys[id] || "nav-solve";
    const current = page === id ? ` aria-current="page"` : "";
    return `<button type="button" data-page="${id}" class="${page === id ? "active" : ""}"${current}>${esc(t(key))}</button>`;
  }
  function presetButton(id) {
    return `<button type="button" data-preset="${id}" aria-pressed="${preset === id}">${esc(t("preset-" + id))}</button>`;
  }
}

function overlays() {
  return `<div id="palette" class="dialog ${paletteOpen ? "" : "hidden"}" role="dialog" aria-modal="true" aria-label="${esc(t("commands"))}"></div>
    <div id="units-pop" class="popover ${unitsOpen ? "" : "hidden"}">${unitsPopover()}</div>`;
}

function unitsPopover(includeApply = true) {
  const u = units();
  const row = (key, label, cycle) => `<label class="field">${esc(t(label))}
    <select data-unit-key="${key}">${cycle.map((item) => `<option ${item === u[key] ? "selected" : ""}>${esc(item)}</option>`).join("")}</select>
  </label>`;
  return `<div class="fields">
    ${row("length", "custom-length", CYCLES.length)}
    ${row("small", "custom-small", CYCLES.length)}
    ${row("angle", "custom-angle", CYCLES.angle)}
    ${row("temperature", "custom-temperature", CYCLES.temperature)}
    ${row("pressure", "custom-pressure", CYCLES.pressure)}
    ${row("velocity", "custom-velocity", CYCLES.velocity)}
    ${row("mass", "custom-mass", CYCLES.mass)}
    ${row("energy", "custom-energy", CYCLES.energy)}
    ${row("angular", "custom-angular", ANGULAR)}
    ${includeApply ? `<button type="button" id="units-apply">${esc(t("apply"))}</button>` : ""}
  </div>`;
}

function pageHtml() {
  if (page === "card") return cardHtml();
  if (page === "charts") return chartHtml();
  if (page === "reticle") return reticleHtml();
  if (page === "targets") return targetsHtml();
  if (page === "pbr") return pbrHtml();
  if (page === "truing") return truingHtml();
  if (page === "equipment") return equipmentHtml();
  if (page === "compare") return compareHtml();
  if (page === "documents") return documentsHtml();
  if (page === "about") return aboutHtml();
  if (page === "settings") return settingsHtml();
  return solveHtml();
}

function weatherBlock() {
  return `<p class="hint">${esc(t("weather-intro"))}</p>
    <p id="air-badge" class="badge-fresh"></p>
    <p id="air-detail" class="meta"></p>
    <div class="row">
      <button type="button" id="weather-fetch" title="${esc(t("weather-fetch-tip"))}" ${weatherBusy ? "disabled" : ""}>${esc(t("weather-fetch"))}</button>
    </div>
    <p class="meta">${boot.settings.weather_place ? `${esc(t("weather-place-using"))} ${esc(boot.settings.weather_place)}` : esc(t("weather-no-place"))}</p>
    <p id="weather-status" class="${weatherKind && weatherKind !== "place" ? "weather-error" : "meta"}">${esc(weatherMessage)}</p>
    ${weatherOffer ? weatherResultHtml(weatherOffer) : ""}
    <p class="hint">${esc(t("weather-attribution"))}</p>
    <p class="hint">${esc(t("weather-noncommercial"))}</p>`;
}

function weatherResultHtml(offer) {
  const u = units();
  const temp = formatLocal(offer.temperature_c, "temperature", u.temperature);
  const station = formatLocal(offer.surface_pressure_pa, "pressure", u.pressure);
  const sea = formatLocal(offer.sea_level_pressure_pa, "pressure", u.pressure);
  const elev = formatLocal(offer.elevation_m, "length", u.length);
  const wind = offer.wind_speed_mps == null ? "—" : `${formatLocal(offer.wind_speed_mps, "velocity", u.velocity)} ${u.velocity}`;
  const gust = offer.wind_gust_mps == null ? "—" : `${formatLocal(offer.wind_gust_mps, "velocity", u.velocity)} ${u.velocity}`;
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

function solveHtml() {
  const s = boot.settings;
  const profiles = boot.equipment.profiles;
  const options = profiles.map((item) => `<option value="${esc(item.id)}" ${item.id === s.active_profile_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const active = profiles.find((item) => item.id === s.active_profile_id);
  return `<div class="layout">
    <section class="panel inputs">
      <label class="field">${esc(t("profile"))}
        <select id="profile">${options || `<option value="">${esc(t("no-profile"))}</option>`}</select>
      </label>
      <p class="meta" id="zero-summary">${active ? esc(active.summary || "") : esc(t("no-profile"))}</p>
      <p class="meta" id="truing-line">${esc((lastSolve && lastSolve.truing) || t("truing-none"))}</p>
      <p class="banner hidden" id="differs">${esc(t("air-differs"))}</p>
      ${qtyField("range", t("range"), s.last_range, s.last_range_unit || units().length, "length", s.last_range_si, s.last_range_display)}
      ${qtyField("wind_speed", t("wind-speed"), s.last_wind_speed, s.last_wind_speed_unit || units().velocity, "velocity", s.last_wind_speed_si, s.last_wind_speed_display)}
      <label class="field">${esc(t("wind-direction"))}
        <input id="wind_direction" value="${esc(s.last_wind_direction || "12oc")}" />
        <span class="hint">${esc(t("wind-clock-hint"))}</span>
      </label>
      ${qtyField("inclination", t("inclination"), s.last_inclination, "deg", "angle", s.last_inclination_si, s.last_inclination_display)}
      <p class="meta">${esc(t("current-air"))}</p>
      ${airFields(s.current_air || {}, "")}
      <div id="weather-block">${weatherBlock()}</div>
      <div class="checks" id="effects">
        ${check("incline", true)}${check("spin_drift", true)}${check("coriolis", false)}${check("cant", false)}${check("aerodynamic_jump", false)}
      </div>
      <div id="extra" class="fields hidden">
        ${qtyField("latitude", t("latitude"), "0 deg", "deg", "angle")}
        ${qtyField("azimuth", t("azimuth"), "0 deg", "deg", "angle")}
        ${qtyField("cant", t("cant-angle"), "0 deg", "deg", "angle")}
      </div>
      <p class="hint">${esc(t("field-suffix-hint"))}</p>
      <p class="hint">${esc(t("spin-note"))}</p>
      <p class="hint">${esc(t("jump-note"))}</p>
      <details class="noprint" id="lead-box" ${moving.open ? "open" : ""}>
        <summary>${esc(t("lead-title"))}</summary>
        <label class="field">${esc(t("lead-preset"))}
          <select id="lead-preset">
            <option value="">${esc(t("overlay-none"))}</option>
            ${(boot.running || []).map((item) => `<option value="${esc(item.id)}" ${moving.preset === item.id ? "selected" : ""}>${esc(item.name)}</option>`).join("")}
          </select>
        </label>
        <p class="hint" id="lead-note">${esc(leadNote())}</p>
        ${qtyField("target_speed", t("target-speed"), moving.speedText, moving.speedUnit || units().velocity, "velocity")}
        <label class="field">${esc(t("lead-direction"))}
          <input id="target_direction" value="${esc(moving.direction || "3oc")}" />
          <span class="hint">${esc(t("lead-clock"))}</span>
        </label>
        <label class="field">${esc(t("lead-reaction"))}
          <input id="reaction_s" value="${esc(moving.reaction || "0")}" inputmode="decimal" />
          <span class="hint">${esc(t("lead-reaction-note"))}</span>
        </label>
        <label class="field" title="${esc(t("lead-target-tip"))}">${esc(t("impact-target"))}
          <select id="lead-target" title="${esc(t("lead-target-tip"))}">
            <option value="">${esc(t("lead-target-none"))}</option>
            ${targetOptions(leadTarget, ["sjf", "svsf"])}
          </select>
        </label>
        <label class="field" title="${esc(t("facing-tip"))}">${esc(t("facing"))}
          <select id="lead-facing" title="${esc(t("facing-tip"))}">
            <option value="left" ${leadFacing === "left" ? "selected" : ""}>${esc(t("facing-left"))}</option>
            <option value="right" ${leadFacing === "right" ? "selected" : ""}>${esc(t("facing-right"))}</option>
          </select>
        </label>
      </details>
    </section>
    <section>
      <div id="hold" class="hold" aria-live="polite">${holdInner()}</div>
      <p class="error" id="solve-error"></p>
      <p class="hint">${esc(t("keys-hint"))}</p>
    </section>
  </div>`;
}

function check(name, fallback) {
  const effects = boot.settings.effects || {};
  const on = effects[name] !== undefined ? effects[name] : fallback;
  const key = name === "spin_drift" ? "effect-spin" : name === "aerodynamic_jump" ? "effect-jump" : "effect-" + name;
  return `<label><input type="checkbox" data-effect="${name}" ${on ? "checked" : ""} /> ${esc(t(key))}</label>`;
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

function airFields(air, prefix) {
  const temperature = airPart(air, "temperature", units().temperature);
  const pressure = airPart(air, "pressure", units().pressure);
  const altitude = airPart(air, "altitude", units().length);
  const humidity = air && typeof air.humidity === "string" ? air.humidity : ((air && air.humidity_text) || "0%");
  return `${qtyField(prefix + "temperature", t("temperature"), temperature.text, temperature.unit, "temperature", temperature.si, temperature.display)}
    ${qtyField(prefix + "pressure", t("pressure"), pressure.text, pressure.unit, "pressure", pressure.si, pressure.display)}
    <label class="field">${esc(t("kind"))}
      <select id="${prefix}pressure_kind">
        <option value="station" ${!air || air.pressure_kind !== "sea-level" ? "selected" : ""}>${esc(t("pressure-station"))}</option>
        <option value="sea-level" ${air && air.pressure_kind === "sea-level" ? "selected" : ""}>${esc(t("pressure-sea"))}</option>
      </select>
    </label>
    <label class="field">${esc(t("humidity"))}
      <input id="${prefix}humidity" value="${esc(humidity)}" />
      <span class="hint">${esc(t("humidity-hint"))}</span>
    </label>
    ${qtyField(prefix + "altitude", t("altitude"), altitude.text, altitude.unit, "length", altitude.si, altitude.display)}`;
}

function qtyField(id, label, text, unit, dim, si, display) {
  const raw = text == null ? "" : String(text);
  const numeric = Number(si);
  const hasSi = si !== undefined && si !== null && si !== "" && Number.isFinite(numeric);
  const shown = display !== undefined && display !== null && String(display) !== ""
    ? String(display)
    : (hasSi ? formatLocal(numeric, dim, unit) : raw);
  return `<label class="field">${esc(label)}
    <span class="entry">
      <input id="${id}" data-dim="${dim}" data-unit="${esc(unit)}" data-text="${esc(raw)}" data-si="${hasSi ? esc(String(numeric)) : ""}" data-dirty="0" value="${esc(shown)}" autocomplete="off" spellcheck="false" />
      <button type="button" class="chip" data-for="${id}">${esc(unit)}</button>
    </span>
    <span class="preview" id="${id}-preview"></span>
  </label>`;
}

function recordField(record, name, label, dim, fallbackUnit) {
  return qtyField(
    name,
    label,
    record[`${name}_text`] || "",
    record[`${name}_unit`] || fallbackUnit,
    dim,
    record[`${name}_si`],
    record[`${name}_display`]
  );
}

function holdInner() {
  const elev = lastSolve && lastSolve.elevation;
  const wind = lastSolve && lastSolve.windage;
  const line = (dir, primary, clicks) => `<div>
    <p class="direction">${esc(dir || "—")}</p>
    <p class="figure">${esc(primary || "—")}</p>
    <p class="clicks">${clicks === null || clicks === undefined ? "" : esc(clicks) + " " + esc(t("clicks"))}</p>
  </div>`;
  const secondary = lastSolve ? [
    `${t("velocity")} ${lastSolve.velocity_display} ${lastSolve.velocity_unit}`,
    `${t("energy")} ${lastSolve.energy_display} ${lastSolve.energy_unit}`,
    `${t("tof")} ${Number(lastSolve.tof_s).toFixed(3)} s`,
    `${t("mach")} ${Number(lastSolve.mach).toFixed(2)}`,
    `${t("drop")} ${lastSolve.drop_display} ${lastSolve.drop_unit}`,
    lastSolve.sg ? `${t("sg")} ${Number(lastSolve.sg).toFixed(2)}` : t("no-sg"),
    `${t("density-altitude")} ${lastSolve.density_altitude_display} ${lastSolve.density_altitude_unit || ""}`,
  ].join(" · ") : "";
  const near = lastSolve && lastSolve.sensitivity ? lastSolve.sensitivity.map((item) => {
    if (item.kind === "range") return `${Number(item.range_m).toFixed(0)} m ${item.elevation.direction} ${item.elevation.primary}`;
    return `${t("wind-speed")} ${Number(item.speed_mps).toFixed(1)} ${item.windage.direction} ${item.windage.primary}`;
  }).join(" · ") : "";
  return `${line(elev && elev.direction, elev && elev.primary, elev && elev.clicks)}
    ${line(wind && wind.direction, wind && wind.primary, wind && wind.clicks)}
    <p class="secondary">${esc(secondary)}</p>
    <p class="sensitivity">${esc(near)}</p>
    <p class="warning" id="warns">${lastSolve ? esc([].concat(lastSolve.warnings || [], lastSolve.station_pressure_note || []).join(" ")) : ""}</p>
    ${leadHold()}`;
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
  return `<section class="card">
    <div class="row noprint">
      ${qtyField("from", t("card-from"), "100 m", units().length, "length", 100)}
      ${qtyField("to", t("card-to"), "600 m", units().length, "length", 600)}
      ${qtyField("step", t("card-step"), "50 m", units().length, "length", 50)}
      <label class="field">${esc(t("card-winds"))}<input id="brackets" value="${esc(cardBrackets)}" /></label>
      <label class="field">${esc(t("card-size"))}<select id="card-page">${pages}</select></label>
      <label class="field">${esc(t("card-conditions"))}<input id="conditions" value="${esc(cardConditions)}" placeholder="-10 C, 30 C" /></label>
      <button type="button" id="card-build">${esc(t("apply"))}</button>
      <button type="button" id="card-pdf" title="${esc(t("card-pdf-tip"))}">${esc(t("card-pdf"))}</button>
      <button type="button" id="card-print" title="${esc(t("card-print-note"))}">${esc(t("card-print"))}</button>
      <button type="button" id="card-csv">${esc(t("card-csv"))}</button>
      <button type="button" id="card-json">${esc(t("card-json"))}</button>
      <label><input type="checkbox" id="card-pin" ${s.card_pinned ? "checked" : ""} /> ${esc(t("card-pin"))}</label>
    </div>
    <p class="hint noprint">${esc(t("page-pin-note"))} ${esc(t("card-paper-note"))}</p>
    <p class="hint noprint">${esc(t("card-print-note"))}</p>
    <p class="hint noprint" id="card-pdf-status"></p>
    <div id="card-body">${card && card.rows ? cardTable(card) : `<p>${esc(t("card-empty"))}</p>`}</div>
  </section>`;
}

function cardTable(data) {
  const heads = data.brackets.map((label) => `<th>${esc(label)}</th>`).join("");
  const conditionHeads = (data.conditions || []).map((label) => `<th>${esc(label)}</th>`).join("");
  const rows = data.rows.map((row) => `<tr>
    <td>${esc(row.range_display)} ${esc(data.length_unit)}</td>
    <td>${esc(row.elevation.direction)} ${esc(row.elevation.primary)}</td>
    <td>${esc(row.drop_display)}</td>
    <td>${esc(row.velocity_display)}</td>
    <td>${esc(row.energy_display)}</td>
    <td>${Number(row.tof_s).toFixed(3)}</td>
    <td>${Number(row.mach).toFixed(2)}</td>
    ${(row.winds || []).map((wind) => `<td>${esc(wind.windage.direction)} ${esc(wind.windage.primary)}</td>`).join("")}
    ${(row.conditions || []).map((item) => `<td>${esc(item.elevation.direction)} ${esc(item.elevation.primary)}</td>`).join("")}
  </tr>`).join("");
  const barMm = Number(data.scale_bar_mm);
  const barLabel = esc(data.scale_bar_label || "");
  const bar = barMm
    ? `<div class="scale-bar scale-bar-screen" style="width:${barMm}mm"><span>${barLabel}</span></div>
       <div class="scale-bar scale-bar-print"><span>${barLabel}</span></div>`
    : "";
  return `<h2>${esc(data.profile_label || "")}</h2>
    <p>${esc(data.zero_summary || "")}</p>
    <p class="meta">${esc(t("truing"))}: ${esc(data.truing || t("truing-none"))} · ${esc(data.date || "")}</p>
    <p class="meta">${esc(t("from-zero"))}. ${esc(t("crosswind"))}.</p>
    ${bar}
    <table><thead><tr>
      <th>${esc(t("range"))}</th><th>${esc(t("cards-elevation"))}</th><th>${esc(t("drop"))}</th>
      <th>${esc(t("velocity"))}</th><th>${esc(t("energy"))}</th><th>${esc(t("tof"))}</th><th>${esc(t("mach"))}</th>
      ${heads}${conditionHeads}
    </tr></thead><tbody>${rows}</tbody></table>
    <p class="disclaimer">${esc(data.footer || t("range-card-footer"))}</p>
    <p class="disclaimer">${esc(data.disclaimer || boot.disclaimer)}</p>`;
}

function chartHtml() {
  const profiles = boot.equipment.profiles;
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
  return `<section class="panel">
    <div class="checks noprint">${boxes}</div>
    <div class="checks noprint">${kinds}</div>
    <div class="row noprint">
      <button type="button" id="chart-build">${esc(t("apply"))}</button>
      <button type="button" id="chart-svg">${esc(t("download-svg"))}</button>
      <button type="button" id="chart-png">${esc(t("download-png"))}</button>
      <button type="button" id="chart-csv">${esc(t("card-csv"))}</button>
    </div>
    <p class="hint">${esc(t("charts-note"))} ${esc(t("same-conditions"))} ${esc(t("chart-limit"))}</p>
    <div id="chart-body"></div>
    <h3>${esc(t("impact-title"))}</h3>
    <p class="hint">${esc(t("impact-intro"))}</p>
    <div class="fields noprint">
      <label class="field" title="${esc(t("impact-target-tip"))}">${esc(t("impact-target"))}
        <select id="impact-target" title="${esc(t("impact-target-tip"))}">
          ${targetOptions(impactTarget)}
          <option value="custom" ${impactTarget === "custom" ? "selected" : ""}>${esc(t("impact-custom"))}</option>
        </select>
      </label>
      <label class="field" title="${esc(t("facing-tip"))}">${esc(t("facing"))}
        <select id="impact-facing" title="${esc(t("facing-tip"))}">
          <option value="left" ${impactFacing === "left" ? "selected" : ""}>${esc(t("facing-left"))}</option>
          <option value="right" ${impactFacing === "right" ? "selected" : ""}>${esc(t("facing-right"))}</option>
        </select>
      </label>
      <label class="field" title="${esc(t("ringing-tip"))}">${esc(t("ringing"))}
        <select id="impact-ringing" title="${esc(t("ringing-tip"))}">
          <option value="primary" ${impactRinging === "primary" ? "selected" : ""}>${esc(t("ringing-primary"))}</option>
          <option value="alt" ${impactRinging === "alt" ? "selected" : ""}>${esc(t("ringing-alt"))}</option>
        </select>
      </label>
      <label title="${esc(t("silhouette-tip"))}"><input id="impact-silhouette" type="checkbox" ${impactSilhouette ? "checked" : ""} /> ${esc(t("silhouette"))}</label>
      <span title="${esc(t("impact-range-tip"))}">${qtyField("impact-range", t("range"), boot.settings.last_range || "300 m", units().length, "length")}</span>
      <span title="${esc(t("impact-mv-tip"))}">${qtyField("mv_sd", t("impact-mv-sd"), "0 mps", units().velocity, "velocity")}</span>
      <span title="${esc(t("impact-error-tip"))}">${qtyField("range_error", t("impact-range-error"), "1 m", units().length, "length", 1)}</span>
      <label class="field" title="${esc(t("impact-wind-tip"))}">${esc(t("card-winds"))}
        <input id="impact-brackets" value="2 mps, 4 mps" title="${esc(t("impact-wind-tip"))}" />
      </label>
      <span title="${esc(t("impact-zone-tip"))}">${qtyField("impact-zone", t("pbr-zone"), "20 cm", units().small, "length")}</span>
      <button type="button" id="impact-draw" title="${esc(t("impact-draw-tip"))}">${esc(t("impact-draw"))}</button>
    </div>
    <div id="impact-body"></div>
  </section>`;
}

function equipmentHtml() {
  const tabs = ["rifle", "scope", "bullet", "load", "profile", "chrono", "recoil", "log", "archive"].map((kind) =>
    `<button type="button" data-kind="${kind}" class="${equipKind === kind ? "active" : ""}">${esc(t("equip-" + kind))}</button>`).join("");
  if (equipKind === "archive") {
    return `<section class="panel"><div class="row">${tabs}</div>${archiveHtml()}</section>`;
  }
  const items = equipList().map((item) =>
    `<button type="button" data-pick="${esc(item.id)}">${esc(item.label)}</button>`).join("");
  const library = equipKind === "recoil" ? "" : (items || `<p>${esc(t("equip-empty"))}</p>`);
  const newer = equipKind === "recoil" || equipKind === "chrono" ? "" : `<button type="button" id="equip-new">${esc(t("new"))}</button>`;
  return `<div class="layout">
    <section class="panel list"><div class="row">${tabs}</div>${newer}${library}</section>
    <section class="panel" id="equip-form">${equipForm()}</section>
  </div>`;
}

function equipList() {
  if (equipKind === "log") return boot.equipment.load_logs || [];
  return boot.equipment[equipKind + "s"] || boot.equipment[equipKind] || [];
}

function currentRecord() {
  const list = equipKind === "log" ? (boot.equipment.load_logs || []) : (boot.equipment[equipKind + "s"] || []);
  return list.find((item) => item.id === selectedId) || null;
}

function equipForm() {
  if (equipKind === "chrono") return chronoHtml();
  if (equipKind === "recoil") return recoilHtml();
  if (equipKind === "log") return logForm();
  const record = currentRecord() || {};
  const id = record.id ? `<input type="hidden" id="record-id" value="${esc(record.id)}" />` : `<input type="hidden" id="record-id" value="" />`;
  let fields = "";
  if (equipKind === "rifle") {
    fields = textInput("label", t("label"), record.label) + textInput("make_model", t("make-model"), record.make_model)
      + recordField(record, "barrel_length", t("barrel"), "length", units().small)
      + textInput("twist", t("twist"), record.twist_text)
      + `<label class="field">${esc(t("twist"))}<select id="twist_hand"><option value="right" ${record.twist_hand !== "left" ? "selected" : ""}>RH</option><option value="left" ${record.twist_hand === "left" ? "selected" : ""}>LH</option></select></label>`
      + recordField(record, "sight_height", t("sight-height"), "length", units().small)
      + recordField(record, "weight", t("weight"), "mass", units().mass)
      + textInput("serial", t("serial"), record.serial) + `<p class="hint">${esc(t("serial-note"))}</p>`;
  } else if (equipKind === "scope") {
    fields = textInput("label", t("label"), record.label) + textInput("make_model", t("make-model"), record.make_model)
      + `<label class="field">${esc(t("focal-plane"))}<select id="focal_plane"><option value="ffp" ${record.focal_plane !== "sfp" ? "selected" : ""}>${esc(t("ffp"))}</option><option value="sfp" ${record.focal_plane === "sfp" ? "selected" : ""}>${esc(t("sfp"))}</option></select></label>`
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
      + `<label class="field">${esc(t("drag-model"))}<select id="drag_model">${["G1", "G7", "G2", "G5", "G6", "G8"].map((model) => `<option ${record.drag_model === model ? "selected" : ""}>${model}</option>`).join("")}</select></label>`
      + numberInput("bc", t("bc"), bands[0] ? bands[0].bc : "")
      + `<label class="field">${esc(t("bc-reference"))}<select id="bc_reference"><option ${record.bc_reference !== "Army" ? "selected" : ""}>ICAO</option><option ${record.bc_reference === "Army" ? "selected" : ""}>Army</option></select></label>`
      + numberInput("bc2", t("bc-second"), second.bc ?? "")
      + qtyField("bc2_min", t("bc-min"), second.min_text || "", "mps", "velocity")
      + `<p class="hint">${esc(t("bc-hint"))} ${esc(t("band-hint"))}</p>`
      + textInput("source_note", t("source-note"), record.source_note);
  } else if (equipKind === "load") {
    const bullets = boot.equipment.bullets.map((item) => `<option value="${esc(item.id)}" ${item.id === record.bullet_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
    fields = textInput("label", t("label"), record.label)
      + `<label class="field">${esc(t("bullet-link"))}<select id="bullet_id">${bullets}</select></label>`
      + recordField(record, "mv", t("mv"), "velocity", units().velocity)
      + recordField(record, "mv_temperature", t("mv-temperature"), "temperature", units().temperature)
      + recordField(record, "sensitivity", t("powder-sensitivity"), "velocity", units().velocity)
      + `<p class="hint">${esc(t("powder-hint"))}</p>`
      + textInput("source_note", t("source-note"), record.source_note);
  } else {
    const select = (id, label, list, current) => `<label class="field">${esc(label)}<select id="${id}">${list.map((item) => `<option value="${esc(item.id)}" ${item.id === current ? "selected" : ""}>${esc(item.label)}</option>`).join("")}</select></label>`;
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
  return `${id}<div class="fields">${fields}</div>
    <div class="row"><button type="button" id="equip-save">${esc(t("save"))}</button>
    ${record.id ? `<button type="button" id="equip-delete">${esc(t("archive"))}</button>` : ""}</div>
    <p id="equip-msg" class="meta">${esc(notice)}</p>`;
}

function textInput(id, label, value) {
  return `<label class="field">${esc(label)}<input id="${id}" value="${esc(value || "")}" /></label>`;
}

function numberInput(id, label, value) {
  return `<label class="field">${esc(label)}<input id="${id}" value="${esc(value ?? "")}" inputmode="decimal" /></label>`;
}

function settingsHtml() {
  const s = boot.settings;
  const rings = boot.targets || { moose_cm: [], roe_cm: [] };
  const lat = s.weather_latitude == null ? "" : String(s.weather_latitude);
  const lon = s.weather_longitude == null ? "" : String(s.weather_longitude);
  return `<section class="panel fields">
    <p class="hint">${esc(t("settings-page-intro"))}</p>
    <h2>${esc(t("settings-appearance"))}</h2>
    <div class="checks">
      ${["follow", "light", "dark"].map((mode) => `<label><input type="radio" name="appearance" value="${mode}" ${s.appearance === mode ? "checked" : ""} /> ${esc(t("settings-" + mode))}</label>`).join("")}
    </div>
    <p class="hint">${esc(t("follow-note"))}</p>
    <label><input type="checkbox" id="large-text" ${s.large_text ? "checked" : ""} /> ${esc(t("settings-large"))}</label>
    <label><input type="checkbox" id="reduced-motion" ${s.reduced_motion ? "checked" : ""} /> ${esc(t("settings-motion"))}</label>
    <p class="meta">${esc(t("palette"))}: ${esc(theme.source || "")} · ${esc(t("text-scale"))} ${esc(theme.text_scale || 1)}</p>
    <h2>${esc(t("settings-weather"))}</h2>
    <p class="hint">${esc(t("settings-weather-intro"))}</p>
    <label class="field">${esc(t("weather-place"))}
      <input id="weather-place" value="${esc(s.weather_place || "")}" title="${esc(t("weather-place-tip"))}" />
    </label>
    <label class="field">${esc(t("weather-latitude"))}
      <input id="weather-lat" value="${esc(lat)}" inputmode="decimal" title="${esc(t("weather-latitude-tip"))}" />
    </label>
    <label class="field">${esc(t("weather-longitude"))}
      <input id="weather-lon" value="${esc(lon)}" inputmode="decimal" title="${esc(t("weather-longitude-tip"))}" />
    </label>
    <label class="field">${esc(t("weather-stale-minutes"))}
      <input id="weather-stale" value="${esc(s.weather_stale_minutes || 60)}" inputmode="numeric" title="${esc(t("weather-stale-tip"))}" />
    </label>
    <button type="button" id="weather-save" title="${esc(t("weather-save-tip"))}">${esc(t("weather-save"))}</button>
    <p id="weather-save-msg" class="meta"></p>
    <p class="hint">${esc(t("weather-attribution"))}</p>
    <p class="hint">${esc(t("weather-noncommercial"))}</p>
    <h2>${esc(t("settings-library"))}</h2>
    <p class="hint">${esc(t("settings-library-intro"))}</p>
    <label><input type="checkbox" id="include-location" title="${esc(t("include-location-tip"))}" /> ${esc(t("include-location"))}</label>
    <div class="row">
      <button type="button" id="export">${esc(t("settings-export"))}</button>
      <button type="button" id="export-csv" title="${esc(t("settings-library-intro"))}">${esc(t("settings-export-csv"))}</button>
      <button type="button" id="show-json">${esc(t("open-backup"))}</button>
    </div>
    <p id="backup-path" class="meta"></p>
    <pre id="backup-json" class="hidden"></pre>
    <p class="hint">${esc(t("include-note"))}</p>
    <label class="field">${esc(t("settings-restore"))}<input id="restore-file" type="file" accept="application/json,.json,text/csv,.csv" title="${esc(t("settings-library-intro"))}" /></label>
    <pre id="restore-preview"></pre>
    <div class="row">
      <button type="button" id="restore-commit" class="hidden">${esc(t("settings-replace"))}</button>
      <button type="button" id="restore-merge" class="hidden">${esc(t("import-merge"))}</button>
    </div>
    <p class="hint">${esc(t("restore-warning"))}</p>
    <h2>${esc(t("targets-title"))}</h2>
    <p>${esc(t("rings-moose"))}: ${esc((rings.moose_cm || []).join(" / "))} cm</p>
    <p>${esc(t("rings-roe"))}: ${esc((rings.roe_cm || []).join(" / "))} cm</p>
    <p class="hint">${esc(t("targets-note"))} ${esc(t("older-rings"))}</p>
    <p class="hint">${esc(boot.spin_provenance || "")}</p>
    <h2>${esc(t("nav-about"))}</h2>
    <p class="hint">${esc(t("settings-about"))}</p>
    <button type="button" id="open-about-settings">${esc(t("nav-about"))}</button>
  </section>`;
}

function onboardHtml() {
  return `<main class="onboard">
    <h1>${esc(t("onboard-title"))}</h1>
    <div class="choices">
      <button type="button" data-onboard="metric"><strong>${esc(t("onboard-metric"))}</strong>${esc(t("onboard-metric-detail"))}</button>
      <button type="button" data-onboard="us"><strong>${esc(t("onboard-us"))}</strong>${esc(t("onboard-us-detail"))}</button>
      <button type="button" data-onboard="custom"><strong>${esc(t("onboard-custom"))}</strong>${esc(t("onboard-custom-detail"))}</button>
    </div>
    <label><input type="checkbox" id="example" checked /> ${esc(t("onboard-example"))}</label>
    <div id="onboard-custom" class="hidden">${unitsPopover(false)}</div>
    <p class="hint">${esc(t("example-note"))}</p>
    <p class="disclaimer">${esc(boot.disclaimer)}</p>
    <p class="hint" id="onboard-custom-note"></p>
    <button type="button" id="onboard-accept" disabled>${esc(t("onboard-accept"))}</button>
  </main>`;
}

function bindOnboard() {
  let preset = "";
  document.querySelectorAll("[data-onboard]").forEach((button) => {
    button.addEventListener("click", () => {
      preset = button.dataset.onboard;
      document.querySelectorAll("[data-onboard]").forEach((item) => item.classList.toggle("active", item === button));
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
  document.getElementById("units-open").addEventListener("click", () => {
    unitsOpen = !unitsOpen;
    render();
  });
  const apply = document.getElementById("units-apply");
  if (apply) {
    apply.addEventListener("click", async () => {
      const custom = {};
      document.querySelectorAll("[data-unit-key]").forEach((select) => { custom[select.dataset.unitKey] = select.value; });
      boot = await call("set_preferences", { body: { preset: "custom", units: custom, angular: custom.angular, appearance: boot.settings.appearance, large_text: boot.settings.large_text, reduced_motion: boot.settings.reduced_motion, card_pinned: boot.settings.card_pinned } });
      unitsOpen = false;
      render();
    });
  }
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
    });
    input.addEventListener("change", () => {
      if (page === "solve") scheduleSolve(0);
      if (page === "compare") scheduleCompare(0);
    });
  });
  if (page === "solve") bindSolve();
  if (page === "card") bindCard();
  if (page === "charts") bindChart();
  if (page === "reticle") bindReticle();
  if (page === "targets") bindTargets();
  if (page === "pbr") bindPbr();
  if (page === "truing") bindTruing();
  if (page === "equipment") bindEquipment();
  if (page === "compare") bindCompare();
  if (page === "documents") bindDocuments();
  if (page === "settings") bindSettings();
}

function bindSolve() {
  ["profile", "wind_direction", "pressure_kind", "humidity"].forEach((id) => {
    const node = document.getElementById(id);
    if (!node) return;
    node.addEventListener("input", () => {
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
        speed.value = moving.speedText;
        speed.dataset.unit = "mps";
        speed.dataset.dirty = "1";
        const chip = document.querySelector('[data-for="target_speed"]');
        if (chip) chip.textContent = "mps";
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
  const details = document.getElementById("lead-box");
  if (details) details.addEventListener("toggle", () => { moving.open = details.open; });
  paintLeadFace();
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
    host.innerHTML = `<p class="error">${esc(error)}</p>`;
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
  const onImpactControl = (id, node) => {
    if (id === "impact-target") impactTarget = node.value;
    if (id === "impact-facing") impactFacing = node.value;
    if (id === "impact-ringing") impactRinging = node.value;
    redrawImpact();
  };
  ["impact-target", "impact-facing", "impact-ringing"].forEach((id) => {
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
    host.innerHTML = `<p>${esc(t("chart-empty"))}</p>`;
    return;
  }
  try {
    chartData = await call("chart", { body });
    chartCursor = null;
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
  chartPainting = false;
}

function legendItems() {
  return (chartData.series || []).map((item, index) =>
    `<span><i class="swatch swatch-${index % 6}"></i>${esc(item.label)}</span>`).join("");
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
        return `${item.label}: ${signed(point.path)} ${chartData.drop.unit} · ${signed(point.elevation)} ${chartData.drop.hold_unit}`;
      }
      if (kind === "wind") {
        return `${item.label}: ${signed(point.windage)} ${chartData.wind.unit} · ${signed(point.wind_linear)} ${chartData.wind.linear_unit}`;
      }
      return `${item.label}: ${signed(point[yKey])} ${spec.unit || ""}`;
    }).filter(Boolean);
    node.textContent = `${signed(range)} ${chartData.x.unit} — ${parts.join("  ·  ")}`;
  });
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
    render();
  }));
  document.querySelectorAll("[data-pick]").forEach((button) => button.addEventListener("click", () => {
    selectedId = button.dataset.pick;
    render();
  }));
  const newer = document.getElementById("equip-new");
  if (newer) newer.addEventListener("click", () => { selectedId = ""; render(); });
  const save = document.getElementById("equip-save");
  if (save) save.addEventListener("click", saveEquipment);
  const remove = document.getElementById("equip-delete");
  if (remove) remove.addEventListener("click", deleteEquipment);
  if (equipKind === "chrono") bindChrono();
  if (equipKind === "recoil") bindRecoil();
  if (equipKind === "log") bindLog();
  if (equipKind === "archive") bindArchive();
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
    await call("delete_record", { kind: equipKind, id: selectedId });
    selectedId = "";
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

function bindSettings() {
  const about = document.getElementById("open-about-settings");
  if (about) about.addEventListener("click", () => { page = "about"; render(); });
  document.querySelectorAll("input[name=appearance]").forEach((input) => input.addEventListener("change", async () => {
    boot = await call("set_preferences", { body: preferencesBody({ appearance: input.value }) });
  }));
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
  const result = await call(command, { include_location: includeLocation() });
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
  chip.textContent = next;
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
}

function showPreview(input) {
  const preview = document.getElementById(`${input.id}-preview`);
  if (!preview) return;
  const parsed = parseLocal(input.value, input.dataset.dim, input.dataset.unit);
  if (!parsed || parsed.bare) {
    preview.textContent = "";
    return;
  }
  const shown = formatLocal(parsed.si, input.dataset.dim, input.dataset.unit);
  preview.textContent = `${input.value.trim()} ${t("preview-arrow")} ${shown} ${input.dataset.unit}`;
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
  const error = document.getElementById("solve-error");
  if (!hold) return;
  try {
    const body = solveBody();
    if (!body.profile_id) {
      error.textContent = t("no-profile");
      return;
    }
    lastSolve = await call("solve", { body });
    stale = false;
    solveError = "";
    hold.classList.remove("stale");
    hold.innerHTML = holdInner();
    paintLeadFace();
    error.textContent = "";
    const differs = document.getElementById("differs");
    if (differs) differs.classList.toggle("hidden", !lastSolve.current_differs);
    const summary = document.getElementById("zero-summary");
    if (summary) summary.textContent = lastSolve.zero_summary || "";
  } catch (err) {
    stale = true;
    solveError = String(err);
    hold.classList.add("stale");
    error.textContent = `${t("last-valid")} ${solveError}`;
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
  if (!suffix) return { si: toSi(value, dim, unit), bare: true };
  return { si: toSi(value, dim, suffix), bare: false };
}

function toSi(value, dim, unit) {
  const u = String(unit || "").toLowerCase();
  if (dim === "temperature") {
    if (u === "f") return (value - 32) * 5 / 9;
    if (u === "k") return value - 273.15;
    return value;
  }
  const factors = {
    m: 1, mm: 0.001, cm: 0.01, in: 0.0254, ft: 0.3048, yd: 0.9144,
    g: 0.001, kg: 1, gr: 0.00006479891, lb: 0.45359237, oz: 0.028349523125,
    mps: 1, "m/s": 1, fps: 0.3048, mph: 0.44704, "km/h": 1000 / 3600, kn: 1852 / 3600,
    hpa: 100, mbar: 100, inhg: 3386.389, mmhg: 3386.389 / 25.4, psi: 6894.757293168361, pa: 1,
    mrad: 0.001, mil: 0.001, moa: Math.PI / (180 * 60), deg: Math.PI / 180,
    iphy: 0.001 / 3.6, "cm@100m": 0.0001,
    j: 1, ftlb: 0.3048 * 0.45359237 * 9.80665,
  };
  return value * (factors[u] || 1);
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
  if (key === "r") focusId("range");
  else if (key === "w") focusId("wind_speed");
  else if (key === "d") focusId("wind_direction");
  else if (key === "a") focusId("inclination");
  else if (key === "u") cycleAngular();
  else if (event.key === "ArrowUp" || event.key === "ArrowDown") { event.preventDefault(); stepRange(event.key === "ArrowUp" ? 1 : -1, event); }
  else if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); stepClock(event.key === "ArrowRight" ? 1 : -1); }
  else if (event.key === "+" || event.key === "=") { event.preventDefault(); stepWind(1); }
  else if (event.key === "-" || event.key === "_") { event.preventDefault(); stepWind(-1); }
});

function focusId(id) {
  const node = document.getElementById(id);
  if (node) node.focus();
}

async function cycleAngular() {
  const index = ANGULAR.indexOf(boot.settings.angular || "mrad");
  const angular = ANGULAR[(index + 1) % ANGULAR.length];
  boot = await call("set_preferences", { body: preferencesBody({ angular }) });
  scheduleSolve(0);
  const hold = document.getElementById("hold");
  if (hold && lastSolve) {
    lastSolve.angular = angular;
  }
}

function stepRange(sign, event) {
  const input = document.getElementById("range");
  if (!input) return;
  const parsed = parseLocal(input.value, "length", input.dataset.unit) || { si: 0 };
  let factor = 1;
  if (event.shiftKey) factor *= 4;
  if (event.ctrlKey) factor *= 0.1;
  const step = (parseLocal(boot.settings.range_step, "length", "m") || { si: 10 }).si * factor * sign;
  const next = Math.max(1, parsed.si + step);
  input.value = formatLocal(next, "length", input.dataset.unit);
  input.dataset.dirty = "1";
  input.dataset.text = `${input.value} ${input.dataset.unit}`;
  scheduleSolve(0);
}

function stepClock(sign) {
  const input = document.getElementById("wind_direction");
  if (!input) return;
  const match = String(input.value).match(/(\d+)/);
  let hour = match ? Number(match[1]) : 12;
  hour = (hour + sign + 12) % 12;
  if (hour === 0) hour = 12;
  input.value = `${hour}oc`;
  scheduleSolve(0);
}

function stepWind(sign) {
  const input = document.getElementById("wind_speed");
  if (!input) return;
  const parsed = parseLocal(input.value, "velocity", input.dataset.unit) || { si: 0 };
  const windStep = (parseLocal(boot.settings.wind_step, "velocity", "mps") || { si: 1 }).si;
  const next = Math.max(0, parsed.si + windStep * sign);
  input.value = formatLocal(next, "velocity", input.dataset.unit);
  input.dataset.dirty = "1";
  scheduleSolve(0);
}

async function copyHold() {
  if (!lastSolve) return;
  const text = `${lastSolve.elevation.direction} ${lastSolve.elevation.primary} · ${lastSolve.windage.direction} ${lastSolve.windage.primary}`;
  try {
    await navigator.clipboard.writeText(text);
  } catch (error) {
    const node = document.getElementById("solve-error");
    if (node) node.textContent = t("copy-failed");
  }
}

function fillPalette() {
  const box = document.getElementById("palette");
  if (!box) return;
  const items = [
    ["solve", t("nav-solve")],
    ["card", t("nav-card")],
    ["charts", t("nav-charts")],
    ["reticle", t("nav-reticle")],
    ["targets", t("nav-targets")],
    ["pbr", t("nav-pbr")],
    ["truing", t("nav-truing")],
    ["equipment", t("nav-equipment")],
    ["compare", t("nav-compare")],
    ["documents", t("nav-documents")],
    ["about", t("nav-about")],
    ["settings", t("nav-settings")],
    ["metric", t("command-metric")],
    ["us", t("command-us")],
    ["custom", t("command-custom")],
    ["field", t("command-field")],
    ["copy", t("command-copy")],
  ];
  box.innerHTML = `<p>${esc(t("commands"))}</p>` + items.map(([id, label]) => `<button type="button" data-cmd="${id}">${esc(label)}</button>`).join("");
  box.querySelectorAll("[data-cmd]").forEach((button) => button.addEventListener("click", () => runCommand(button.dataset.cmd)));
  const first = box.querySelector("button");
  if (first) first.focus();
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
  return `<section class="panel">
    <div class="checks">
      <label title="${esc(t("pbr-optimal-tip"))}"><input type="radio" name="pbr-mode" value="optimal" ${pbrMode !== "fixed" ? "checked" : ""} /> ${esc(t("pbr-optimal"))}</label>
      <label title="${esc(t("pbr-fixed-tip"))}"><input type="radio" name="pbr-mode" value="fixed" ${pbrMode === "fixed" ? "checked" : ""} /> ${esc(t("pbr-fixed"))}</label>
    </div>
    <label class="field">${esc(t("pbr-preset"))}
      <select id="pbr-preset">
        <option value="moose" ${pbrZonePreset === "moose" ? "selected" : ""}>${esc(t("pbr-moose"))}</option>
        <option value="roe" ${pbrZonePreset === "roe" ? "selected" : ""}>${esc(t("pbr-roe"))}</option>
        <option value="custom" ${pbrZonePreset === "custom" ? "selected" : ""}>${esc(t("impact-custom"))}</option>
      </select>
    </label>
    ${qtyField("zone", t("pbr-zone"), pbrZoneText(), pbrZoneUnit(), "length")}
    <div class="${pbrMode === "fixed" ? "" : "hidden"}">${qtyField("pbr-zero", t("pbr-zero-input"), pbrZeroText, units().length, "length")}</div>
    <button type="button" id="pbr-build">${esc(t("apply"))}</button>
    <p class="hint">${esc(pbrMode === "fixed" ? "pbr-fixed-note" : "pbr-note")}</p>
    <div id="pbr-body">${pbrResult ? pbrView(pbrResult) : ""}</div>
  </section>`;
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
  return `<p><strong>${esc(t("pbr-start-label"))}</strong> ${esc(data.near_display)} ${esc(data.length_unit)}</p>
    <p class="hint">${esc(t("pbr-start-help"))}</p>
    ${inside}
    <p><strong>${esc(t("pbr-near-zero-label"))}</strong> ${esc(data.near_zero_display)} ${esc(data.length_unit)}</p>
    <p class="hint">${esc(t("pbr-near-zero-help"))}</p>
    <p><strong>${esc(t("pbr-far-zero-label"))}</strong> ${esc(data.far_zero_display)} ${esc(data.length_unit)}</p>
    <p class="hint">${esc(t("pbr-far-zero-help"))}</p>
    <p><strong>${esc(t("pbr-end-label"))}</strong> ${esc(data.far_display)} ${esc(data.length_unit)}</p>
    <p class="hint">${esc(t("pbr-end-help"))}</p>
    ${above}
    <p><strong>${esc(t("pbr-ordinate"))}</strong> ${esc(data.ordinate_display)} ${esc(t("pbr-at"))} ${esc(data.ordinate_range_display)} ${esc(data.length_unit)}</p>
    <p class="hint">${esc(t("pbr-ordinate-help"))}</p>
    <div class="row" id="pbr-spans">${pbrSpanButtons(data)}</div>
    <p class="hint">${esc(t("pbr-zoom-hint"))}</p>
    <div id="pbr-chart"></div>
    <p class="hint">${esc(t("pbr-still-air"))}</p>
    ${confirm}`;
}

function pbrSpanButtons(data) {
  const unit = (data.x && data.x.unit) || data.length_unit || "m";
  const choices = [
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
  const band = (value) => `<line class="band" x1="${padL}" x2="${width - padR}" y1="${yOf(value).toFixed(1)}" y2="${yOf(value).toFixed(1)}" />`;
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
  const rows = history.map((entry) => `<li>${esc(entry.summary || "")}
    <button type="button" data-history="${esc(entry.id)}" data-action="activate">${esc(t("truing-activate"))}</button>
    <button type="button" data-history="${esc(entry.id)}" data-action="revert">${esc(t("truing-revert"))}</button>
  </li>`).join("");
  return `<section class="panel fields">
    <p>${esc(t("truing-intro"))}</p>
    <p class="meta">${esc(t("truing-active"))}: ${esc(active)}</p>
    <label class="field" title="${esc(t("truing-kind-tip"))}">${esc(t("truing-kind"))}
      <select id="truing-kind" title="${esc(t("truing-kind-tip"))}">
        <option value="mv" ${truingKind === "mv" ? "selected" : ""}>${esc(t("truing-mv"))}</option>
        <option value="drag" ${truingKind === "drag" ? "selected" : ""}>${esc(t("truing-drag"))}</option>
        <option value="tracking" ${truingKind === "tracking" ? "selected" : ""}>${esc(t("truing-tracking"))}</option>
      </select>
    </label>
    <div id="truing-fields">${truingFields()}</div>
    <button type="button" id="truing-preview">${esc(t("settings-preview"))}</button>
    <div id="truing-result">${truingPreview ? truingDiff(truingPreview) : ""}</div>
    <h3>${esc(t("truing-history"))}</h3>
    <ul>${rows || `<li>${esc(t("truing-history-empty"))}</li>`}</ul>
    <p class="hint">${esc(t("truing-confirm-note"))}</p>
  </section>`;
}

function truingFields() {
  if (truingKind === "drag") {
    return `<label class="field" title="${esc(t("truing-lines-tip"))}">${esc(t("truing-lines"))}<textarea id="truing-lines" rows="5" title="${esc(t("truing-lines-tip"))}">${esc(truingForm.lines)}</textarea></label>
      <p class="hint">${esc(t("truing-lines-hint"))}</p>
      <label title="${esc(t("truing-piecewise-tip"))}"><input type="checkbox" id="truing-piecewise" ${truingForm.piecewise ? "checked" : ""} /> ${esc(t("truing-piecewise"))}</label>
      <p class="hint">${esc(t("truing-piecewise-tip"))}</p>
      <p class="hint">${esc(t("truing-drag-guidance"))}</p>`;
  }
  if (truingKind === "tracking") {
    return `<span title="${esc(t("truing-advertised-tip"))}">${qtyField("advertised", t("truing-advertised"), truingForm.advertised, "mrad", "angle")}</span>
      <p class="hint">${esc(t("truing-advertised-tip"))}</p>
      <span title="${esc(t("truing-observed-tip"))}">${qtyField("observed", t("truing-observed"), truingForm.observed, "mrad", "angle")}</span>
      <p class="hint">${esc(t("truing-observed-tip"))}</p>
      <p class="hint">${esc(t("truing-tracking-guidance"))}</p>`;
  }
  return `<span title="${esc(t("truing-range-tip"))}">${qtyField("observe_range", t("range"), truingForm.range, units().length, "length")}</span>
    <p class="hint">${esc(t("truing-range-tip"))}</p>
    <span title="${esc(t("truing-elev-tip"))}">${qtyField("observe_elevation", t("elevation"), truingForm.elevation, "mrad", "angle")}</span>
    <p class="hint">${esc(t("truing-elev-tip"))}</p>
    <label class="field" title="${esc(t("truing-shots-tip"))}">${esc(t("truing-shots"))}<input id="truing-shots" value="${esc(truingForm.shots)}" title="${esc(t("truing-shots-tip"))}" /></label>
    <p class="hint">${esc(t("truing-shots-tip"))}</p>
    <label class="field" title="${esc(t("truing-group-tip"))}">${esc(t("truing-group"))}<input id="truing-group" value="${esc(truingForm.group)}" title="${esc(t("truing-group-tip"))}" /></label>
    <p class="hint">${esc(t("truing-group-tip"))}</p>
    <p class="hint">${esc(t("truing-mv-guidance"))}</p>`;
}

function truingDiff(preview) {
  const detail = preview.detail || {};
  const warnings = [];
  if (detail.large_change) warnings.push(t("truing-large"));
  if (detail.before_transonic === false && preview.kind === "mv") warnings.push(t("truing-transonic"));
  if (detail.guidance) warnings.push(detail.guidance);
  const apply = preview.applied ? "" : `<button type="button" id="truing-apply">${esc(t("truing-apply"))}</button>`;
  const changed = preview.kind === "drag"
    ? t("truing-result-drag")
    : preview.kind === "tracking"
      ? t("truing-result-tracking")
      : t("truing-result-mv");
  return `<p>${esc(preview.summary || preview.action || "")}</p>
    <p class="hint">${esc(changed)}</p>
    <p class="warning">${esc(warnings.join(" "))}</p>
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
    if (host) host.innerHTML = truingFields();
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
  const result = recoilResult ? `<p>${esc(t("recoil-velocity"))} ${esc(recoilResult.velocity_display)} ${esc(recoilResult.velocity_unit)} · ${esc(t("energy"))} ${esc(recoilResult.energy_display)} ${esc(recoilResult.energy_unit)}</p>
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
    if (host) host.innerHTML = recoilHtml().includes("unused") ? "" : `<p>${esc(t("recoil-velocity"))} ${esc(recoilResult.velocity_display)} ${esc(recoilResult.velocity_unit)} · ${esc(t("energy"))} ${esc(recoilResult.energy_display)} ${esc(recoilResult.energy_unit)}</p><p>${esc(recoilResult.convention || "")}</p>${recoilResult.reduction_is_estimate ? `<p class="hint">${esc(t("recoil-estimate"))}</p>` : ""}`;
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

function flagHtml(flags) {
  return (flags || []).map((flag) => {
    const kind = t("badge-" + (flag.kind || "placeholder"));
    const detail = t(flag.detail || ("flag-" + flag.code));
    return `<p class="badge badge-${esc(flag.kind || "placeholder")}">${esc(kind)} · ${esc(flag.code || "")} · ${esc(detail)}</p>`;
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
    const dashed = path.placeholder ? 'stroke-dasharray="8 5"' : "";
    const stroke = path.placeholder ? "var(--warning)" : ink;
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
  const barCm = half > 30 ? 10 : 5;
  const bar0 = px(-barCm / 2, 0);
  const bar1 = px(barCm / 2, 0);
  const step = half > 80 ? 20 : half > 40 ? 10 : 5;
  let ticks = "";
  for (let cm = Math.ceil(-half / step) * step; cm <= half; cm += step) {
    const [x] = px(cm, 0);
    ticks += `<line stroke="${axis}" x1="${x.toFixed(1)}" x2="${x.toFixed(1)}" y1="${(cy - 6).toFixed(1)}" y2="${(cy + 6).toFixed(1)}" />
      <text fill="${ink}" x="${x.toFixed(1)}" y="${(cy + 24).toFixed(1)}" text-anchor="middle">${esc(lengthLabelFromCm(cm))}</text>`;
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
  return `<section class="panel">
    <h2>${esc(t("reticle-title"))}</h2>
    <p class="hint">${esc(t("reticle-intro"))}</p>
    <div class="fields">
      <label class="field" title="${esc(t("reticle-pick-tip"))}">${esc(t("reticle-pick"))}
        <select id="reticle-pick" title="${esc(t("reticle-pick-tip"))}">${reticleChoices()}</select>
      </label>
      <label class="field" title="${esc(t("reticle-mag-tip"))}">${esc(t("reticle-mag"))}
        <input id="reticle-mag" value="${esc(reticleState.mag)}" inputmode="decimal" title="${esc(t("reticle-mag-tip"))}" />
      </label>
      <div class="row">
        ${[5, 10, 20].map((mag) => `<button type="button" data-mag="${mag}" title="${esc(t("reticle-mag-check"))}">${mag}×</button>`).join("")}
      </div>
      <label class="field" title="${esc(t("reticle-fov-tip"))}">${esc(t("reticle-fov"))}
        <input id="reticle-fov" value="${esc(reticleState.fov)}" inputmode="decimal" title="${esc(t("reticle-fov-tip"))}" />
      </label>
      <label class="field" title="${esc(t("reticle-focal-tip"))}">${esc(t("reticle-focal"))}
        <select id="reticle-focal" title="${esc(t("reticle-focal-tip"))}">
          <option value="ffp" ${reticleState.focal === "ffp" ? "selected" : ""}>${esc(t("focal-ffp"))}</option>
          <option value="sfp" ${reticleState.focal === "sfp" ? "selected" : ""}>${esc(t("focal-sfp"))}</option>
        </select>
      </label>
      <label class="field" title="${esc(t("reticle-cal-tip"))}">${esc(t("reticle-cal"))}
        <input id="reticle-cal" value="${esc(reticleState.cal)}" inputmode="decimal" title="${esc(t("reticle-cal-tip"))}" />
      </label>
      <label class="field" title="${esc(t("impact-target-tip"))}">${esc(t("impact-target"))}
        <select id="reticle-target" title="${esc(t("impact-target-tip"))}">
          <option value="">${esc(t("reticle-no-target"))}</option>
          ${targetOptions(reticleState.target)}
        </select>
      </label>
      <span title="${esc(t("reticle-range-tip"))}">${qtyField("reticle-range", t("range"), reticleState.rangeText, units().length, "length")}</span>
      <label class="field" title="${esc(t("facing-tip"))}">${esc(t("facing"))}
        <select id="reticle-facing" title="${esc(t("facing-tip"))}">
          <option value="left" ${reticleState.facing === "left" ? "selected" : ""}>${esc(t("facing-left"))}</option>
          <option value="right" ${reticleState.facing === "right" ? "selected" : ""}>${esc(t("facing-right"))}</option>
        </select>
      </label>
      <label class="field" title="${esc(t("ringing-tip"))}">${esc(t("ringing"))}
        <select id="reticle-ringing" title="${esc(t("ringing-tip"))}">
          <option value="primary" ${reticleState.ringing !== "alt" ? "selected" : ""}>${esc(t("ringing-primary"))}</option>
          <option value="alt" ${reticleState.ringing === "alt" ? "selected" : ""}>${esc(t("ringing-alt"))}</option>
        </select>
      </label>
      <label title="${esc(t("reticle-hold-tip"))}"><input id="reticle-hold" type="checkbox" ${reticleState.showHold ? "checked" : ""} /> ${esc(t("reticle-hold"))}</label>
      <label title="${esc(t("silhouette-tip"))}"><input id="reticle-silhouette" type="checkbox" ${reticleState.showSilhouette ? "checked" : ""} /> ${esc(t("silhouette"))}</label>
    </div>
    <p id="reticle-readout" class="hint"></p>
    <p id="reticle-warn" class="badge hidden"></p>
    <div id="reticle-flags"></div>
    <div id="reticle-face"></div>
    <h3>${esc(t("editor-title"))}</h3>
    <p class="hint">${esc(t("editor-intro"))}</p>
    <div class="fields">
      <label class="field" title="${esc(t("editor-name-tip"))}">${esc(t("editor-name"))}
        <input id="reticle-name" title="${esc(t("editor-name-tip"))}" />
      </label>
      <label class="field" title="${esc(t("editor-tool-tip"))}">${esc(t("editor-tool"))}
        <select id="reticle-tool" title="${esc(t("editor-tool-tip"))}">
          ${["line", "hash", "dot", "circle", "text"].map((tool) => `<option value="${tool}" ${reticleState.tool === tool ? "selected" : ""}>${esc(t("tool-" + tool))}</option>`).join("")}
        </select>
      </label>
      <label class="field" title="${esc(t("editor-text-tip"))}">${esc(t("editor-text"))}
        <input id="reticle-text" value="1" title="${esc(t("editor-text-tip"))}" />
      </label>
      <label class="field" title="${esc(t("editor-snap-tip"))}">${esc(t("editor-snap"))}
        <input id="reticle-snap" value="${esc(reticleState.snap)}" inputmode="decimal" title="${esc(t("editor-snap-tip"))}" />
      </label>
      <label class="field" title="${esc(t("editor-repeat-tip"))}">${esc(t("editor-repeat"))}
        <input id="reticle-copies" value="3" inputmode="decimal" title="${esc(t("editor-repeat-tip"))}" />
      </label>
      <label class="field" title="${esc(t("editor-dx-tip"))}">${esc(t("editor-dx"))}
        <input id="reticle-dx" value="1" inputmode="decimal" title="${esc(t("editor-dx-tip"))}" />
      </label>
      <label class="field" title="${esc(t("editor-dy-tip"))}">${esc(t("editor-dy"))}
        <input id="reticle-dy" value="0" inputmode="decimal" title="${esc(t("editor-dy-tip"))}" />
      </label>
    </div>
    <div class="row">
      <button type="button" id="reticle-mirror-x" title="${esc(t("editor-mirror-x-tip"))}">${esc(t("editor-mirror-x"))}</button>
      <button type="button" id="reticle-mirror-y" title="${esc(t("editor-mirror-y-tip"))}">${esc(t("editor-mirror-y"))}</button>
      <button type="button" id="reticle-repeat" title="${esc(t("editor-repeat-tip"))}">${esc(t("editor-repeat-apply"))}</button>
      <button type="button" id="reticle-undo" title="${esc(t("editor-undo-tip"))}">${esc(t("editor-undo"))}</button>
      <button type="button" id="reticle-save" title="${esc(t("editor-save-tip"))}">${esc(t("editor-save"))}</button>
      <button type="button" id="reticle-export" title="${esc(t("editor-export-tip"))}">${esc(t("editor-export"))}</button>
      <label class="field" title="${esc(t("editor-import-tip"))}">${esc(t("editor-import"))}<input id="reticle-import" type="file" accept="application/json,.json" title="${esc(t("editor-import-tip"))}" /></label>
    </div>
    <p id="reticle-notice" class="hint">${esc(reticleNotice)}</p>
    <p class="hint">${esc(t("reticle-pdf-note"))}</p>
  </section>`;
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
    for (let at = c; at < size; at += stepPx) {
      grid += gridLine(at, c, size, true) + gridLine(size - (at - c), c, size, true);
    }
    for (let at = c; at < size; at += stepPx) {
      grid += gridLine(at, c, size, false) + gridLine(size - (at - c), c, size, false);
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
    <text fill="var(--text)" x="${(c + 14).toFixed(1)}" y="${(c - 8).toFixed(1)}">${esc(t(reticleState.showHold ? "reticle-hold-point" : "reticle-centre"))}</text>`;
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

function targetsHtml() {
  const pages = (boot.card_pages || []).map((item) => `<option value="${esc(item.id)}" ${reticleState.page === item.id ? "selected" : ""}>${esc(item.name)}</option>`).join("");
  const spec = currentTarget();
  return `<section class="panel">
    <h2>${esc(t("targets-page-title"))}</h2>
    <p class="hint">${esc(t("targets-intro"))}</p>
    <h3>${esc(t("open-items"))}</h3>
    <p class="hint">${esc(t("open-items-intro"))}</p>
    <div id="open-items">${openItemsHtml()}</div>
    <div class="fields">
      <label class="field" title="${esc(t("impact-target-tip"))}">${esc(t("impact-target"))}
        <select id="target-pick" title="${esc(t("impact-target-tip"))}">${targetOptions(targetPickId())}</select>
      </label>
      <label class="field" title="${esc(t("facing-tip"))}">${esc(t("facing"))}
        <select id="target-facing" title="${esc(t("facing-tip"))}">
          <option value="left" ${targetFacing !== "right" ? "selected" : ""}>${esc(t("facing-left"))}</option>
          <option value="right" ${targetFacing === "right" ? "selected" : ""}>${esc(t("facing-right"))}</option>
        </select>
      </label>
      <label class="field" title="${esc(t("ringing-tip"))}">${esc(t("ringing"))}
        <select id="target-ringing" title="${esc(t("ringing-tip"))}">
          <option value="primary" ${targetRinging !== "alt" ? "selected" : ""}>${esc(t("ringing-primary"))}</option>
          <option value="alt" ${targetRinging === "alt" ? "selected" : ""}>${esc(t("ringing-alt"))}</option>
        </select>
      </label>
      <label title="${esc(t("silhouette-tip"))}"><input id="target-silhouette" type="checkbox" ${targetSilhouette ? "checked" : ""} /> ${esc(t("silhouette"))}</label>
      <label class="field" title="${esc(t("card-size"))}">${esc(t("card-size"))}
        <select id="target-page" title="${esc(t("card-size"))}">${pages}</select>
      </label>
    </div>
    <div id="target-flags">${spec ? flagHtml(spec.flags) : ""}</div>
    <p class="hint">${spec ? esc(spec.subtitle || "") : ""} ${spec && spec.distance_note ? "· " + esc(spec.distance_note) : ""}</p>
    <div id="target-face">${spec ? faceSvg(spec, { showSilhouette: true, ringing: "primary", label: spec.name }) : ""}</div>
    <h3>${esc(t("target-edit-title"))}</h3>
    <p class="hint">${esc(t("target-edit-intro"))}</p>
    <div id="target-editor">${targetEditorHtml(spec)}</div>
    <div class="row">
      <button type="button" id="target-save" title="${esc(t("target-save-tip"))}">${esc(t("target-save"))}</button>
      <button type="button" id="target-pdf" title="${esc(t("target-pdf-tip"))}">${esc(t("target-pdf"))}</button>
    </div>
    <p id="target-notice" class="hint">${esc(targetNotice)}</p>
    <h3>${esc(t("svg-title"))}</h3>
    <p class="hint">${esc(t("svg-intro"))}</p>
    <div class="fields">
      <label class="field" title="${esc(t("svg-file-tip"))}">${esc(t("svg-file"))}<input id="svg-file" type="file" accept=".svg,image/svg+xml" title="${esc(t("svg-file-tip"))}" /></label>
      <span title="${esc(t("svg-width-tip"))}">${qtyField("svg-width", t("svg-width"), "20 cm", units().small, "length")}</span>
      <span title="${esc(t("svg-height-tip"))}">${qtyField("svg-height", t("svg-height"), "20 cm", units().small, "length")}</span>
      <button type="button" id="svg-save" title="${esc(t("svg-save-tip"))}">${esc(t("svg-save"))}</button>
    </div>
  </section>`;
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
  const seen = new Set();
  const rows = [];
  ((boot.targets && boot.targets.library) || []).forEach((item) => {
    (item.flags || []).forEach((flag) => {
      if (!flag.detail || seen.has(flag.detail) || flag.code === "T1") return;
      seen.add(flag.detail);
      rows.push(`<p class="badge badge-${esc(flag.kind)}">${esc(flag.code)} · ${esc(t(flag.detail))}</p>`);
    });
  });
  return rows.join("");
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
  const picks = profiles.map((item) => `<label><input type="checkbox" class="compare-pick" value="${esc(item.id)}" ${compareIds.includes(item.id) ? "checked" : ""} /> ${esc(item.label)}</label>`).join("");
  const flags = compareFlags();
  const effect = (name, key) => `<label><input type="checkbox" data-compare-effect="${name}" ${flags[name] ? "checked" : ""} /> ${esc(t(key))}</label>`;
  return `<section class="panel">
    <h2>${esc(t("nav-compare"))}</h2>
    <p class="hint">${esc(t("compare-intro"))}</p>
    <div class="checks">${picks || `<p>${esc(t("compare-empty"))}</p>`}</div>
    <div class="row">
      ${qtyField("compare-range", t("range"), "", units().length, "length", compareForm.rangeSi)}
      ${qtyField("compare-wind", t("wind-speed"), "", units().velocity, "velocity", compareForm.windSi)}
      <label class="field">${esc(t("wind-direction"))}
        <input id="compare-direction" value="${esc(compareForm.direction || "9oc")}" title="${esc(t("compare-wind-tip"))}" />
      </label>
    </div>
    <div class="checks">
      ${effect("incline", "effect-incline")}
      ${effect("spin_drift", "effect-spin")}
      ${effect("coriolis", "effect-coriolis")}
      ${effect("cant", "effect-cant")}
      ${effect("aerodynamic_jump", "effect-jump")}
    </div>
    <p class="hint">${esc(boot.spin_provenance || "")}</p>
    <p class="hint">${esc(t("jump-note"))}</p>
    <div id="compare-result"></div>
  </section>`;
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
    node.innerHTML = `<p class="hint">${esc(t("compare-need"))}</p>`;
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

function compareResultHtml(result) {
  const rows = (result && result.rows) || [];
  if (!rows.length) return `<p class="hint">${esc(t("compare-need"))}</p>`;
  const head = (cells) => cells.map((cell) => `<th scope="col">${esc(cell)}</th>`).join("");
  const descriptive = rows.map((row) => `<tr>
    <th scope="row">${esc(row.label)}</th>
    <td>${esc(row.rifle)}</td><td>${esc(row.barrel)}</td><td>${esc(row.twist)} ${esc(row.twist_hand)}</td>
    <td>${esc(row.sight_height)}</td><td>${esc(row.weight)}</td><td>${esc(row.scope)}</td>
    <td>${esc(row.focal_plane)}</td><td>${esc(row.click)}</td><td>${esc(row.bullet)}</td>
    <td>${esc(row.drag)}</td><td>${esc(row.bc)}</td><td>${esc(row.bullet_weight)}</td>
    <td>${esc(row.load)}</td><td>${esc(row.mv)}</td><td>${esc(row.zero)}</td><td>${esc(row.truing)}</td>
  </tr>`).join("");
  const ballistic = rows.map((row) => `<tr>
    <th scope="row">${esc(row.label)}</th>
    <td>${esc(row.drop)}</td>
    <td>${esc(row.elevation.direction)} ${esc(row.elevation.primary)}</td>
    <td>${esc(row.windage.direction)} ${esc(row.windage.primary)}</td>
    <td>${esc(row.velocity)}</td><td>${esc(row.energy)}</td>
    <td>${esc(Number(row.tof_s).toFixed(3))} s</td><td>${esc(Number(row.mach).toFixed(2))}</td>
  </tr>`).join("");
  const warnings = rows.flatMap((row) => (row.warnings || []).map((item) => `${row.label}: ${item}`));
  const need = rows.length < 2 ? `<p class="hint">${esc(t("compare-need"))}</p>` : "";
  const limit = compareError ? `<p class="error">${esc(compareError)}</p>` : "";
  return `${limit}${need}
    <div class="table-wrap"><table class="words"><caption>${esc(t("compare-descriptive"))}</caption>
      <thead><tr><th scope="col">${esc(t("profile"))}</th>${head([t("equip-rifle"), t("barrel"), t("twist"), t("sight-height"), t("weight"), t("equip-scope"), t("focal-plane"), t("click"), t("equip-bullet"), t("compare-drag"), t("compare-bc"), t("weight"), t("equip-load"), t("mv"), t("zero"), t("compare-truing")])}</tr></thead>
      <tbody>${descriptive}</tbody></table></div>
    <div class="table-wrap"><table class="words"><caption>${esc(t("compare-ballistic"))}</caption>
      <thead><tr><th scope="col">${esc(t("profile"))}</th>${head([t("drop"), t("elevation"), t("windage"), t("velocity"), t("energy"), t("tof"), t("mach")])}</tr></thead>
      <tbody>${ballistic}</tbody></table></div>
    ${warnings.length ? `<p class="hint">${esc(warnings.join(" "))}</p>` : ""}`;
}

function documentsHtml() {
  const docs = boot.equipment.documents || [];
  const current = docs.find((item) => item.id === docId) || {};
  const list = docs.map((item) => `<button type="button" data-doc="${esc(item.id)}">${esc(item.title || item.label)}</button>`).join("") || `<p>${esc(t("documents-none"))}</p>`;
  const loads = (boot.equipment.loads || []).map((item) => `<option value="${esc(item.id)}" ${item.id === current.load_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const rifles = (boot.equipment.rifles || []).map((item) => `<option value="${esc(item.id)}" ${item.id === current.rifle_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const bullets = (boot.equipment.bullets || []).map((item) => `<option value="${esc(item.id)}" ${item.id === current.bullet_id ? "selected" : ""}>${esc(item.label)}</option>`).join("");
  const attached = current.filename ? `<p class="meta">${esc(t("documents-attached"))}: ${esc(current.filename)}</p>` : "";
  return `<div class="layout">
    <section class="panel list"><button type="button" id="doc-new">${esc(t("new"))}</button>${list}</section>
    <section class="panel">
      <h2>${esc(t("nav-documents"))}</h2>
      <p class="hint">${esc(t("documents-intro"))}</p>
      <input type="hidden" id="doc-id" value="${esc(current.id || "")}" />
      <div class="fields">
        ${textInput("doc-title", t("documents-title"), current.title || "")}
        ${textInput("doc-edition", t("documents-edition"), current.edition || "")}
        ${textInput("doc-date", t("documents-date"), current.edition_date || "")}
        <label class="field">${esc(t("documents-link"))}<input id="doc-link" value="${esc(current.link_url || "")}" title="${esc(t("documents-link-tip"))}" /></label>
        <label class="field">${esc(t("documents-file"))}<input id="doc-file" type="file" accept="application/pdf,.pdf" title="${esc(t("documents-file-tip"))}" /></label>
        ${attached}
        <label class="field">${esc(t("equip-load"))}<select id="doc-load"><option value=""></option>${loads}</select></label>
        <label class="field">${esc(t("equip-rifle"))}<select id="doc-rifle"><option value=""></option>${rifles}</select></label>
        <label class="field">${esc(t("equip-bullet"))}<select id="doc-bullet"><option value=""></option>${bullets}</select></label>
        ${textInput("doc-notes", t("log-notes"), current.notes || "")}
      </div>
      <div class="row">
        <button type="button" id="doc-save">${esc(t("save"))}</button>
        ${current.id ? `<button type="button" id="doc-open">${esc(t("documents-open"))}</button>` : ""}
        ${current.id ? `<button type="button" id="doc-archive">${esc(t("archive"))}</button>` : ""}
      </div>
      <p id="doc-msg" class="meta">${esc(docNotice)}</p>
    </section>
  </div>`;
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
    </div>
    <div class="row"><button type="button" id="equip-save">${esc(t("save"))}</button>
    ${record.id ? `<button type="button" id="equip-delete">${esc(t("archive"))}</button>` : ""}</div>
    <p id="equip-msg" class="meta">${esc(notice)}</p>`;
}

function chronoRow(item, checked) {
  const mean = Number.isFinite(Number(item.mean_mps)) ? `${formatLocal(Number(item.mean_mps), "velocity", units().velocity)} ${units().velocity}` : "";
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
  const keys = { rifle: "equip-rifle", scope: "equip-scope", bullet: "equip-bullet", load: "equip-load", profile: "equip-profile", chrono: "equip-chrono", reticle: "nav-reticle", target: "nav-targets", document: "nav-documents", load_log: "equip-log", atmosphere: "zero-air" };
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
  return `<section class="panel">
    <h2>${esc(t("nav-about"))}</h2>
    <p class="hint">${esc(t("about-intro"))}</p>
    <p>${esc(t("about-version"))} ${esc(boot.version || "")}</p>
    <h2>${esc(t("about-mit-heading"))}</h2>
    <p>${esc(t("about-mit"))}</p>
    <pre class="licence">${esc(boot.licence || "")}</pre>
    <h2>${esc(t("about-weather-heading"))}</h2>
    <p>${esc(t("weather-attribution"))}</p>
    <p class="hint">${esc(t("weather-noncommercial"))}</p>
    <p>${esc(t("about-personal"))}</p>
    <h2>${esc(t("about-drag-heading"))}</h2>
    <p>${esc(t("about-drag"))}</p>
    <h2>${esc(t("about-crates-heading"))}</h2>
    <p>${esc(t("about-crates"))}</p>
    <h2>${esc(t("about-limits-heading"))}</h2>
    <p>${esc(t("about-limits"))}</p>
    <p class="hint">${esc(boot.spin_provenance || "")}</p>
    <p class="hint">${esc(boot.ejecta_convention || "")}</p>
    <p class="hint">${esc(t("about-keys"))}</p>
    <p class="hint">${esc(t("about-package"))}</p>
  </section>`;
}

listenTheme();
refresh().catch((error) => {
  document.getElementById("app").textContent = String(error);
});
