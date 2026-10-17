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
  applyTheme();
  render();
}

function render() {
  const app = document.getElementById("app");
  if (!boot.settings.onboarded) {
    app.innerHTML = onboardHtml();
    bindOnboard();
    return;
  }
  app.innerHTML = `${shellHtml()}<div class="page" id="page">${pageHtml()}</div><footer class="footer disclaimer">${esc(boot.disclaimer)}</footer>${overlays()}`;
  bindShell();
  bindPage();
  if (page === "solve") scheduleSolve(0);
}

function shellHtml() {
  const preset = boot.settings.preset;
  return `<div class="shell"><header class="top">
    <span class="brand">${esc(t("app-name"))}</span>
    <nav>
      ${nav("solve")}${nav("card")}${nav("charts")}${nav("equipment")}${nav("settings")}
    </nav>
    <div class="presets">
      ${presetButton("metric")}${presetButton("us")}${presetButton("custom")}
      <button type="button" id="units-open">${esc(t("units-button"))}</button>
    </div>
  </header>`;
  function nav(id) {
    const key = id === "card" ? "nav-card" : id === "charts" ? "nav-charts" : id === "equipment" ? "nav-equipment" : id === "settings" ? "nav-settings" : "nav-solve";
    return `<button type="button" data-page="${id}" class="${page === id ? "active" : ""}">${esc(t(key))}</button>`;
  }
  function presetButton(id) {
    return `<button type="button" data-preset="${id}" aria-pressed="${preset === id}">${esc(t("preset-" + id))}</button>`;
  }
}

function overlays() {
  return `<div id="palette" class="dialog ${paletteOpen ? "" : "hidden"}"></div>
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
  if (page === "equipment") return equipmentHtml();
  if (page === "settings") return settingsHtml();
  return solveHtml();
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
      <p class="meta">${esc(t("atmosphere-manual"))} · ${esc(t("truing-none"))}</p>
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
      <p class="hint">${esc(t("lead-later"))}</p>
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
    <p class="warning" id="warns">${lastSolve ? esc([].concat(lastSolve.warnings || [], lastSolve.station_pressure_note || []).join(" ")) : ""}</p>`;
}

function cardHtml() {
  const s = boot.settings;
  return `<section class="card">
    <div class="row noprint">
      ${qtyField("from", t("card-from"), "100 m", units().length, "length", 100)}
      ${qtyField("to", t("card-to"), "600 m", units().length, "length", 600)}
      ${qtyField("step", t("card-step"), "50 m", units().length, "length", 50)}
      <label class="field">${esc(t("card-winds"))}<input id="brackets" value="2 mps, 4 mps, 6 mps" /></label>
      <button type="button" id="card-build">${esc(t("apply"))}</button>
      <button type="button" id="card-print">${esc(t("card-print"))}</button>
      <label><input type="checkbox" id="card-pin" ${s.card_pinned ? "checked" : ""} /> ${esc(t("card-pin"))}</label>
    </div>
    <p class="hint noprint">${esc(t("page-pin-note"))}</p>
    <div id="card-body">${card && card.rows ? cardTable(card) : `<p>${esc(t("card-empty"))}</p>`}</div>
  </section>`;
}

function cardTable(data) {
  const heads = data.brackets.map((label) => `<th>${esc(label)}</th>`).join("");
  const rows = data.rows.map((row) => `<tr>
    <td>${esc(row.range_display)} ${esc(data.length_unit)}</td>
    <td>${esc(row.elevation.direction)} ${esc(row.elevation.primary)}</td>
    <td>${esc(row.drop_display)}</td>
    <td>${esc(row.velocity_display)}</td>
    <td>${esc(row.energy_display)}</td>
    <td>${Number(row.tof_s).toFixed(3)}</td>
    <td>${Number(row.mach).toFixed(2)}</td>
    ${(row.winds || []).map((wind) => `<td>${esc(wind.windage.direction)} ${esc(wind.windage.primary)}</td>`).join("")}
  </tr>`).join("");
  return `<h2>${esc(data.profile_label || "")}</h2>
    <p>${esc(data.zero_summary || "")}</p>
    <p class="meta">${esc(t("from-zero"))}. ${esc(t("crosswind"))}.</p>
    <table><thead><tr>
      <th>${esc(t("range"))}</th><th>${esc(t("cards-elevation"))}</th><th>${esc(t("drop"))}</th>
      <th>${esc(t("velocity"))}</th><th>${esc(t("energy"))}</th><th>${esc(t("tof"))}</th><th>${esc(t("mach"))}</th>
      ${heads}
    </tr></thead><tbody>${rows}</tbody></table>
    <p class="disclaimer">${esc(data.disclaimer || boot.disclaimer)}</p>`;
}

function chartHtml() {
  const profiles = boot.equipment.profiles;
  const boxes = profiles.map((item) => `<label><input type="checkbox" class="overlay" value="${esc(item.id)}" ${item.id === boot.settings.active_profile_id ? "checked" : ""} /> ${esc(item.label)}</label>`).join("");
  return `<section class="panel">
    <div class="checks noprint">${boxes}</div>
    <button type="button" id="chart-build" class="noprint">${esc(t("apply"))}</button>
    <p class="hint">${esc(t("charts-note"))} ${esc(t("same-conditions"))}</p>
    <div id="chart-body"></div>
  </section>`;
}

function equipmentHtml() {
  const tabs = ["rifle", "scope", "bullet", "load", "profile"].map((kind) =>
    `<button type="button" data-kind="${kind}" class="${equipKind === kind ? "active" : ""}">${esc(t("equip-" + kind))}</button>`).join("");
  const items = (boot.equipment[equipKind + "s"] || boot.equipment[equipKind] || []).map((item) =>
    `<button type="button" data-pick="${esc(item.id)}">${esc(item.label)}</button>`).join("");
  return `<div class="layout">
    <section class="panel list"><div class="row">${tabs}</div><button type="button" id="equip-new">${esc(t("new"))}</button>${items || `<p>${esc(t("equip-empty"))}</p>`}</section>
    <section class="panel" id="equip-form">${equipForm()}</section>
  </div>`;
}

function currentRecord() {
  const list = boot.equipment[equipKind + "s"] || [];
  return list.find((item) => item.id === selectedId) || null;
}

function equipForm() {
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
    ${record.id ? `<button type="button" id="equip-delete">${esc(t("delete"))}</button>` : ""}</div>
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
  return `<section class="panel fields">
    <h2>${esc(t("settings-appearance"))}</h2>
    <div class="checks">
      ${["follow", "light", "dark"].map((mode) => `<label><input type="radio" name="appearance" value="${mode}" ${s.appearance === mode ? "checked" : ""} /> ${esc(t("settings-" + mode))}</label>`).join("")}
    </div>
    <p class="hint">${esc(t("follow-note"))}</p>
    <label><input type="checkbox" id="large-text" ${s.large_text ? "checked" : ""} /> ${esc(t("settings-large"))}</label>
    <label><input type="checkbox" id="reduced-motion" ${s.reduced_motion ? "checked" : ""} /> ${esc(t("settings-motion"))}</label>
    <p class="meta">${esc(t("palette"))}: ${esc(theme.source || "")} · ${esc(t("text-scale"))} ${esc(theme.text_scale || 1)}</p>
    <h2>${esc(t("settings-backup"))}</h2>
    <div class="row">
      <button type="button" id="export">${esc(t("settings-export"))}</button>
      <button type="button" id="show-json">${esc(t("open-backup"))}</button>
    </div>
    <p id="backup-path" class="meta"></p>
    <pre id="backup-json" class="hidden"></pre>
    <p class="hint">${esc(t("include-note"))}</p>
    <label class="field">${esc(t("settings-restore"))}<input id="restore-file" type="file" accept="application/json,.json" /></label>
    <pre id="restore-preview"></pre>
    <button type="button" id="restore-commit" class="hidden">${esc(t("settings-replace"))}</button>
    <p class="hint">${esc(t("restore-warning"))}</p>
    <h2>${esc(t("targets-title"))}</h2>
    <p>${esc(t("rings-moose"))}: ${esc((rings.moose_cm || []).join(" / "))} cm</p>
    <p>${esc(t("rings-roe"))}: ${esc((rings.roe_cm || []).join(" / "))} cm</p>
    <p class="hint">${esc(t("targets-note"))} ${esc(t("older-rings"))}</p>
    <p class="hint">${esc(boot.spin_provenance || "")}</p>
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
    input.addEventListener("input", () => { input.dataset.dirty = "1"; showPreview(input); if (page === "solve") scheduleSolve(120); });
    input.addEventListener("change", () => { if (page === "solve") scheduleSolve(0); });
  });
  if (page === "solve") bindSolve();
  if (page === "card") bindCard();
  if (page === "charts") bindChart();
  if (page === "equipment") bindEquipment();
  if (page === "settings") bindSettings();
}

function bindSolve() {
  ["profile", "wind_direction", "pressure_kind", "humidity"].forEach((id) => {
    const node = document.getElementById(id);
    if (node) node.addEventListener("input", () => scheduleSolve(80));
  });
  document.querySelectorAll("[data-effect]").forEach((box) => box.addEventListener("change", () => {
    document.getElementById("extra").classList.toggle("hidden", !needsExtra());
    scheduleSolve(0);
  }));
  document.getElementById("extra").classList.toggle("hidden", !needsExtra());
}

function needsExtra() {
  const flags = effects();
  return flags.coriolis || flags.cant;
}

function bindCard() {
  document.getElementById("card-build").addEventListener("click", buildCard);
  document.getElementById("card-print").addEventListener("click", () => window.print());
  document.getElementById("card-pin").addEventListener("change", async (event) => {
    boot = await call("set_preferences", { body: preferencesBody({ card_pinned: event.target.checked }) });
  });
}

async function buildCard() {
  const host = document.getElementById("card-body");
  try {
    const body = solveBody();
    body.from = qtyPayload("from");
    body.to = qtyPayload("to");
    body.step = qtyPayload("step");
    body.brackets = document.getElementById("brackets").value;
    body.pinned_metric = document.getElementById("card-pin").checked;
    card = await call("range_card", { body });
    host.innerHTML = cardTable(card);
  } catch (error) {
    host.innerHTML = `<p class="error">${esc(error)}</p>`;
  }
}

function bindChart() {
  document.getElementById("chart-build").addEventListener("click", buildChart);
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
  const ids = checked.slice(0, 3);
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
    host.dataset.limit = checked.length > 3 ? "1" : "";
    paintCharts();
  } catch (error) {
    host.innerHTML = `<p class="error">${esc(error)}</p>`;
  }
}

function paintCharts() {
  const host = document.getElementById("chart-body");
  if (!host || !chartData || chartPainting) return;
  const width = Math.max(280, host.clientWidth || 640);
  if (width === chartPaintWidth && host.querySelector("svg")) return;
  chartPainting = true;
  chartPaintWidth = width;
  const note = host.dataset.limit === "1" ? `<p class="hint">${esc(t("chart-limit"))}</p>` : "";
  host.innerHTML = note + drawScaledChart("drop") + drawScaledChart("wind");
  bindChartPointers();
  positionCursor();
  writeReadouts();
  chartPainting = false;
}

function legendItems() {
  return (chartData.series || []).map((item, index) =>
    `<span><i class="swatch swatch-${index}"></i>${esc(item.label)}</span>`).join("");
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
  const yKey = kind === "drop" ? "path" : "windage";
  const stride = xLabelStride(xAxis.ticks.length, plotW);
  const last = xAxis.ticks.length - 1;
  let marks = `<rect class="frame" x="${padL}" y="${padT}" width="${plotW.toFixed(1)}" height="${plotH.toFixed(1)}" fill="none" />`;
  yAxis.ticks.forEach((tick, index) => {
    const y = yOf(tick);
    marks += `<line class="grid" x1="${padL}" x2="${(width - padR).toFixed(1)}" y1="${y.toFixed(1)}" y2="${y.toFixed(1)}" />`;
    marks += `<text class="tick" x="${padL - 8}" y="${(y + 4).toFixed(1)}" text-anchor="end">${esc(yAxis.labels[index])}</text>`;
  });
  xAxis.ticks.forEach((tick, index) => {
    const x = xOf(tick);
    marks += `<line class="grid" y1="${padT}" y2="${(height - padB).toFixed(1)}" x1="${x.toFixed(1)}" x2="${x.toFixed(1)}" />`;
    const crowded = index === last - 1 && last % stride !== 0 && stride > 1;
    if ((index % stride === 0 || index === last) && !crowded) {
      const anchor = index === last ? "end" : index === 0 ? "start" : "middle";
      const labelX = index === 0 ? padL : index === last ? width - padR : x;
      marks += `<text class="tick" x="${labelX.toFixed(1)}" y="${height - 6}" text-anchor="${anchor}">${esc(xAxis.labels[index])}</text>`;
    }
  });
  const zeroY = yOf(0);
  if (zeroY >= padT - 1 && zeroY <= height - padB + 1) {
    marks += `<line class="zero" x1="${padL}" x2="${(width - padR).toFixed(1)}" y1="${zeroY.toFixed(1)}" y2="${zeroY.toFixed(1)}" />`;
    if (spec.zero === "los" && width >= 420) {
      marks += `<text class="zero-label" x="${(width - padR - 4).toFixed(1)}" y="${Math.max(padT + 12, zeroY - 5).toFixed(1)}" text-anchor="end">${esc(t("chart-los"))}</text>`;
    }
  }
  const paths = (chartData.series || []).map((item, index) => {
    const d = (item.points || []).map((point, i) =>
      `${i ? "L" : "M"}${xOf(point.range).toFixed(1)},${yOf(point[yKey]).toFixed(1)}`).join(" ");
    return `<path d="${d}" fill="none" stroke="var(--chart-${index})" stroke-width="2" /><circle class="dot hidden" data-dot="${index}" r="3.5" fill="var(--chart-${index})" cx="0" cy="0" />`;
  }).join("");
  const title = kind === "drop" ? t("chart-drop") : t("chart-wind");
  const yTitle = kind === "drop"
    ? `${spec.unit}, ${t("chart-above-los")}`
    : `${spec.unit}, ${t("chart-wind-sign")}`;
  const clicks = spec.clicks_axis ? `<p class="hint">${esc(t("chart-clicks-axis"))}</p>` : "";
  return `<section class="chart-block">
    <h3>${esc(title)}</h3>
    <p class="legend">${legendItems()}</p>
    <p class="axis-title">${esc(yTitle)}</p>
    <svg class="chart" data-chart="${kind}" viewBox="0 0 ${width} ${height}" role="img" tabindex="0" aria-label="${esc(title)}, ${esc(yTitle)}">
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
    const key = svg.dataset.chart === "drop" ? "path" : "windage";
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
  ["drop", "wind"].forEach((kind) => {
    const node = document.querySelector(`[data-readout="${kind}"]`);
    if (!node || !chartData) return;
    if (chartCursor === null) {
      node.textContent = t("chart-cursor");
      return;
    }
    const range = chartData.series[0].points[chartCursor].range;
    const parts = chartData.series.map((item) => {
      const point = item.points[chartCursor];
      if (!point) return "";
      if (kind === "drop") {
        return `${item.label}: ${signed(point.path)} ${chartData.drop.unit} · ${signed(point.elevation)} ${chartData.drop.hold_unit}`;
      }
      return `${item.label}: ${signed(point.windage)} ${chartData.wind.unit} · ${signed(point.wind_linear)} ${chartData.wind.linear_unit}`;
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
}

async function saveEquipment() {
  const msg = document.getElementById("equip-msg");
  try {
    const body = equipPayload();
    const saved = await call("save_" + equipKind, { body });
    selectedId = saved.id;
    equipNotice = saved.warning ? `${t("saved")} ${saved.warning}` : t("saved");
    boot = await call("bootstrap");
    render();
  } catch (error) {
    if (msg) msg.textContent = String(error);
  }
}

async function deleteEquipment() {
  if (!selectedId || !window.confirm(t("delete-confirm"))) return;
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
  return body;
}

function bindSettings() {
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
  document.getElementById("export").addEventListener("click", async () => {
    const result = await call("export_library");
    document.getElementById("backup-path").textContent = `${t("backup-wrote")} ${result.path}`;
    document.getElementById("backup-json").textContent = JSON.stringify(result.document, null, 2);
  });
  document.getElementById("show-json").addEventListener("click", () => {
    document.getElementById("backup-json").classList.toggle("hidden");
  });
  document.getElementById("restore-file").addEventListener("change", async (event) => {
    const file = event.target.files && event.target.files[0];
    if (!file) return;
    const text = await file.text();
    restoreDoc = JSON.parse(text);
    restorePreview = await call("import_library", { document: restoreDoc, commit: false });
    document.getElementById("restore-preview").textContent = JSON.stringify(restorePreview, null, 2);
    document.getElementById("restore-commit").classList.remove("hidden");
  });
  document.getElementById("restore-commit").addEventListener("click", async () => {
    await call("import_library", { document: restoreDoc, commit: true });
    boot = await call("bootstrap");
    render();
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
  chip.textContent = next;
  if (Number.isFinite(si)) {
    input.dataset.si = String(si);
    input.value = formatLocal(si, input.dataset.dim, next);
  }
  showPreview(input);
  if (page === "solve") scheduleSolve(0);
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
  return {
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
    angular: s.angular || "mrad",
    range_step: storedQuantity(s.range_step || "10 m", units().length),
    wind_step: storedQuantity(s.wind_step || "1 mps", units().velocity),
  };
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
    ["equipment", t("nav-equipment")],
    ["settings", t("nav-settings")],
    ["metric", t("command-metric")],
    ["us", t("command-us")],
    ["custom", t("command-custom")],
    ["field", t("command-field")],
    ["copy", t("command-copy")],
  ];
  box.innerHTML = `<p>${esc(t("commands"))}</p>` + items.map(([id, label]) => `<button type="button" data-cmd="${id}">${esc(label)}</button>`).join("");
  box.querySelectorAll("[data-cmd]").forEach((button) => button.addEventListener("click", () => runCommand(button.dataset.cmd)));
}

function runCommand(id) {
  paletteOpen = false;
  if (PRESETS.includes(id)) setPreset(id);
  else if (id === "field") { fieldMode = !fieldMode; applyTheme(); render(); }
  else if (id === "copy") { copyHold(); render(); }
  else { page = id; render(); }
}

listenTheme();
refresh().catch((error) => {
  document.getElementById("app").textContent = String(error);
});
