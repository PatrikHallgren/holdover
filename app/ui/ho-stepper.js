// ho-stepper.js — framework-free stepper + typed input for Holdover (app/ui).
// Pairs with holdover-b.css (.ho-field/.ho-stepper/.ho-step/.ho-chip).
//
// Markup (render with app.js template strings, all text from en.ftl via t()):
//   <div class="ho-field" data-stepper="range">
//     <div class="head"><label for="range">Range</label><kbd>R</kbd></div>
//     <div class="ho-stepper">
//       <button type="button" class="ho-step" data-dir="-1" tabindex="-1" aria-label="Range minus 10 m" title="Range minus 10 m (Shift ×4, Ctrl ×0.1)">−</button>
//       <input id="range" data-dim="length" data-unit="m" value="300" role="spinbutton" autocomplete="off" spellcheck="false">
//       <button type="button" class="chip ho-chip" data-for="range" title="Length unit">m</button>
//       <button type="button" class="ho-step" data-dir="1" tabindex="-1" aria-label="Range plus 10 m">+</button>
//     </div>
//     <p class="ho-hint" id="range-preview"></p>
//   </div>
//
// Step buttons are tabindex=-1: the keyboard path is the input itself
// (ArrowUp/Down, PageUp/Down), so Tab moves field → unit chip → next field.
// The existing .chip click handler (cycleChip) keeps working unchanged.
//
// Every rule below mirrors app.js (stepRange/stepWind/stepClock) and the
// defaults in app/src-tauri/src/session.rs. Nothing new is invented.

const HO_STEP_RULES = {
  // dim, step source, modifiers, clamp — values in SI unless noted.
  range:          { dim: "length",   step: (s) => s.rangeStepSi ?? 10, shift: 4, ctrl: 0.1, min: 1 },
  wind_speed:     { dim: "velocity", step: (s) => s.windStepSi ?? 1,   shift: 1, ctrl: 1,   min: 0 },
  wind_direction: { clock: true },
  // Approved unit-aware steps. The SI delta is formatted in the field's unit.
  inclination:    { dim: "angle",    step: () => Math.PI / 180, shift: 4, ctrl: 0.1, min: -Math.PI / 2, max: Math.PI / 2 },
  temperature:    { dim: "temperature", step: () => 1, shift: 4, ctrl: 0.1 },
  pressure:       { dim: "pressure", step: () => 100, shift: 4, ctrl: 0.1, min: 0 },
  humidity:       { percent: true, step: () => 5, shift: 4, ctrl: 0.1, min: 0, max: 100 },
  target_speed:   { dim: "velocity", step: () => 0.5, shift: 4, ctrl: 0.1, min: 0 },
  magnification:  { count: true, step: () => 1, shift: 1, ctrl: 1, min: 1 },
};
HO_STEP_RULES.zero_range = HO_STEP_RULES.range;
HO_STEP_RULES["compare-range"] = HO_STEP_RULES.range;
HO_STEP_RULES["reticle-range"] = HO_STEP_RULES.range;
HO_STEP_RULES["impact-range"]  = HO_STEP_RULES.range;
HO_STEP_RULES["observe_range"] = HO_STEP_RULES.range;
HO_STEP_RULES["compare-wind"]  = HO_STEP_RULES.wind_speed;
HO_STEP_RULES["compare-direction"] = HO_STEP_RULES.wind_direction;
HO_STEP_RULES.mag_min = HO_STEP_RULES.magnification;
HO_STEP_RULES.mag_max = HO_STEP_RULES.magnification;
HO_STEP_RULES.calibrated_mag = HO_STEP_RULES.magnification;
HO_STEP_RULES["reticle-mag"] = HO_STEP_RULES.magnification;
HO_STEP_RULES["reticle-cal"] = HO_STEP_RULES.magnification;
HO_STEP_RULES["pbr-zero"] = HO_STEP_RULES.range;
HO_STEP_RULES.from = HO_STEP_RULES.range;
HO_STEP_RULES.to = HO_STEP_RULES.range;
HO_STEP_RULES.step = HO_STEP_RULES.range;

function parsePercent(text) {
  const raw = String(text ?? "").trim().replace("−", "-").replace(",", ".");
  if (!raw) return { percent: 0 };
  const match = raw.match(/^([+-]?(?:\d+(?:\.\d+)?|\.\d+))\s*(%)?$/);
  if (!match) return null;
  const value = Number(match[1]);
  if (!Number.isFinite(value)) return null;
  const percent = match[2] || Math.abs(value) > 1 ? value : value * 100;
  return { percent };
}

function parseCount(text) {
  const raw = String(text ?? "").trim().replace("−", "-").replace(",", ".");
  if (!/^[+-]?(?:\d+(?:\.\d+)?|\.\d+)$/.test(raw)) return null;
  const value = Number(raw);
  return Number.isFinite(value) ? value : null;
}

/**
 * attachSteppers(root, deps)
 *   deps.parse(text, dim, unit) -> {si} | null     (app.js parseLocal)
 *   deps.format(si, dim, unit)  -> string          (app.js formatLocal)
 *   deps.settings()             -> {rangeStepSi, windStepSi}   from boot.settings.range_step / wind_step
 *   deps.changed(input)         -> void            (scheduleSolve(0) / scheduleCompare(0) / …)
 *   deps.label(key, vars)       -> string          (t() for aria-labels; optional)
 */
function attachSteppers(root, deps) {
  root.querySelectorAll(".ho-field[data-stepper]").forEach((field) => {
    const input = field.querySelector("input");
    const rule = HO_STEP_RULES[field.dataset.stepper] || HO_STEP_RULES[input && input.id];
    if (!input || !rule) return;

    const apply = (sign, mods) => {
      if (rule.clock) {
        const m = String(input.value).match(/(\d+)/);
        let hour = m ? Number(m[1]) : 12;
        hour = (hour + sign + 12) % 12 || 12;
        input.value = `${hour} o'clock`;
        syncClock(field, hour);
      } else if (rule.percent) {
        const parsed = parsePercent(input.value);
        if (!parsed || parsed.percent < 0 || parsed.percent > 100) { flagInvalid(field, true); return; }
        let factor = 1;
        if (mods.shift) factor *= rule.shift;
        if (mods.ctrl) factor *= rule.ctrl;
        let next = parsed.percent + rule.step(deps.settings()) * factor * sign;
        next = Math.max(rule.min, Math.min(rule.max, next));
        input.value = `${next}%`;
        input.dataset.text = input.value;
        input.dataset.dirty = "1";
        setBounds(field, next, rule);
      } else if (rule.count) {
        const parsed = parseCount(input.value);
        if (parsed == null) { flagInvalid(field, true); return; }
        let factor = 1;
        if (mods.shift) factor *= rule.shift;
        if (mods.ctrl) factor *= rule.ctrl;
        let next = parsed + rule.step(deps.settings()) * factor * sign;
        if (rule.min != null) next = Math.max(rule.min, next);
        if (rule.max != null) next = Math.min(rule.max, next);
        input.value = String(Math.round(next * 10000) / 10000);
        input.dataset.dirty = "1";
        setBounds(field, next, rule);
      } else {
        const parsed = deps.parse(input.value, rule.dim, input.dataset.unit);
        if (!parsed) { flagInvalid(field, true); return; }       // keep text; last result stays dimmed
        let factor = 1;
        if (mods.shift) factor *= rule.shift;
        if (mods.ctrl) factor *= rule.ctrl;
        let next = parsed.si + rule.step(deps.settings()) * factor * sign;
        if (rule.min != null) next = Math.max(rule.min, next);
        if (rule.max != null) next = Math.min(rule.max, next);
        input.value = deps.format(next, rule.dim, input.dataset.unit);
        input.dataset.si = String(next);
        input.dataset.text = `${input.value} ${input.dataset.unit || ""}`.trim();
        input.dataset.dirty = "1";
        setBounds(field, next, rule);
      }
      flagInvalid(field, false);
      deps.changed(input);
    };

    // Buttons: click = one step, Shift/Ctrl-click = modified step,
    // press-and-hold repeats (400 ms delay, then every 80 ms). Pointer events
    // so the same code serves mouse and touch (M9 Android reuse).
    field.querySelectorAll(".ho-step").forEach((button) => {
      const sign = Number(button.dataset.dir) || 1;
      let delay = 0, timer = 0;
      const stop = () => { clearTimeout(delay); clearInterval(timer); button.classList.remove("repeating"); };
      button.addEventListener("pointerdown", (event) => {
        event.preventDefault();                       // keep focus/caret in the input
        const mods = { shift: event.shiftKey, ctrl: event.ctrlKey };
        apply(sign, mods);
        delay = setTimeout(() => {
          button.classList.add("repeating");
          timer = setInterval(() => apply(sign, mods), 80);
        }, 400);
      });
      ["pointerup", "pointerleave", "pointercancel", "blur"].forEach((name) => button.addEventListener(name, stop));
    });

    // Keyboard inside the field (spinbutton behaviour). ArrowLeft/Right stay
    // caret movement while typing; the page-level shortcuts in app.js only run
    // when focus is NOT in an input, so there is no clash.
    input.addEventListener("keydown", (event) => {
      if (event.key === "ArrowUp" || event.key === "ArrowDown") {
        event.preventDefault();
        apply(event.key === "ArrowUp" ? 1 : -1, { shift: event.shiftKey, ctrl: event.ctrlKey });
      } else if (event.key === "PageUp" || event.key === "PageDown") {
        event.preventDefault();
        apply(event.key === "PageUp" ? 1 : -1, { shift: true, ctrl: false });
      } else if (event.key === "Escape") {
        input.blur();                                 // hands keys back to R/W/D/A + arrows
      }
    });

    // Typed input recomputes on every keystroke; bad text marks the field
    // invalid but never clears the result (app keeps lastSolve, dims it).
    input.addEventListener("input", () => {
      if (rule.clock) {
        flagInvalid(field, !/^\s*\d{1,2}\s*(?:oc|o['’]?clock)?\s*$/i.test(input.value) && !/deg/i.test(input.value));
        deps.changed(input);
        return;
      }
      if (rule.percent) {
        const parsed = parsePercent(input.value);
        const bad = !parsed || parsed.percent < 0 || parsed.percent > 100;
        flagInvalid(field, bad);
        if (!bad) setBounds(field, parsed.percent, rule);
        deps.changed(input);
        return;
      }
      if (rule.count) {
        const parsed = parseCount(input.value);
        flagInvalid(field, parsed == null);
        if (parsed != null) setBounds(field, parsed, rule);
        deps.changed(input);
        return;
      }
      const parsed = deps.parse(input.value, rule.dim, input.dataset.unit);
      flagInvalid(field, !parsed);
      if (parsed) setBounds(field, parsed.si, rule);
      deps.changed(input);
    });

    // Clock picker buttons (role=radio inside role=radiogroup).
    field.querySelectorAll(".ho-clock button[data-hour]").forEach((button) => {
      button.addEventListener("click", () => {
        input.value = `${button.dataset.hour} o'clock`;
        syncClock(field, Number(button.dataset.hour));
        deps.changed(input);
      });
    });
  });

  function setBounds(field, si, rule) {
    const down = field.querySelector('.ho-step[data-dir="-1"]');
    const up = field.querySelector('.ho-step[data-dir="1"]');
    if (down) down.disabled = rule.min != null && si <= rule.min;
    if (up) up.disabled = rule.max != null && si >= rule.max;
  }
  function flagInvalid(field, bad) {
    field.classList.toggle("invalid", bad);
    const input = field.querySelector("input");
    if (input) input.setAttribute("aria-invalid", bad ? "true" : "false");
  }
  function syncClock(field, hour) {
    field.querySelectorAll(".ho-clock button[data-hour]").forEach((b) =>
      b.setAttribute("aria-checked", String(Number(b.dataset.hour) === hour)));
  }
}

// Usage in app.js after render() of a page:
//   attachSteppers(document.getElementById("page"), {
//     parse: parseLocal, format: formatLocal,
//     settings: () => ({
//       rangeStepSi: (parseLocal(boot.settings.range_step, "length", "m") || { si: 10 }).si,
//       windStepSi:  (parseLocal(boot.settings.wind_step, "velocity", "mps") || { si: 1 }).si,
//     }),
//     changed: () => scheduleSolve(0),
//   });
if (typeof window !== "undefined") window.attachSteppers = attachSteppers;
