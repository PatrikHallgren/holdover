const rangeInput = document.getElementById("range");
const windInput = document.getElementById("wind");
const elevation = document.getElementById("elevation");
const detail = document.getElementById("detail");
const error = document.getElementById("error");
const scale = document.getElementById("scale");
const root = document.documentElement;

function applyTheme(palette) {
  if (!palette) return;
  const map = {
    "--background": palette.background,
    "--surface": palette.surface,
    "--text": palette.text,
    "--text-secondary": palette.text_secondary,
    "--border": palette.border,
    "--accent": palette.accent,
  };
  for (const [name, value] of Object.entries(map)) {
    if (value) root.style.setProperty(name, value);
  }
  if (palette.mode === "light") root.style.colorScheme = "light";
  if (typeof palette.text_scale === "number" && palette.text_scale > 0) {
    root.style.setProperty("--text-scale", String(palette.text_scale));
    scale.textContent = `Text scale ${palette.text_scale.toFixed(2)} · ${palette.source || ""}`;
  }
}

async function recalculate() {
  try {
    const point = await window.__TAURI__.core.invoke("solve", {
      range: rangeInput.value,
      wind: windInput.value,
    });
    error.hidden = true;
    const direction = point.elevation_direction === "—" ? "" : `${point.elevation_direction} `;
    elevation.textContent = `${direction}${point.elevation_mrad.toFixed(2)} mrad`;
    const clicks = point.elevation_clicks == null ? "" : ` · ${point.elevation_clicks} clicks`;
    detail.textContent =
      `${point.elevation_moa.toFixed(2)} MOA · ${point.elevation_iphy.toFixed(2)} IPHY · ${point.elevation_cm_per_100m.toFixed(1)} cm/100 m${clicks}` +
      ` · wind ${point.windage_direction} ${Math.abs(point.windage_mrad).toFixed(2)} mrad` +
      ` · ${point.velocity_mps.toFixed(0)} m/s · ${point.energy_j.toFixed(0)} J · ${point.tof_s.toFixed(3)} s · Mach ${point.mach.toFixed(2)}`;
  } catch (err) {
    error.hidden = false;
    error.textContent = String(err);
  }
}

rangeInput.addEventListener("input", recalculate);
windInput.addEventListener("input", recalculate);

window.__TAURI__.event.listen("theme", (event) => applyTheme(event.payload));
recalculate();
