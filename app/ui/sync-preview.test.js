// On a portrait phone the sync preview put Apply sync just past the bottom
// of the screen. Apply and Cancel have to sit in the viewport for an empty
// preview and for a short list that includes a conflict.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(here, "../..");
const app = fs.readFileSync(path.join(here, "app.js"), "utf8");
const css = fs.readFileSync(path.join(here, "app.css"), "utf8")
  + fs.readFileSync(path.join(here, "holdover-b.css"), "utf8");
const ftl = fs.readFileSync(path.join(root, "locales/en.ftl"), "utf8");

function sliceBetween(source, startMark, endMark) {
  const start = source.indexOf(startMark);
  const end = source.indexOf(endMark, start + startMark.length);
  assert.ok(start >= 0 && end > start, startMark);
  return source.slice(start, end);
}

function stringsFromFtl(text) {
  const strings = {};
  text.split("\n").forEach((line) => {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) return;
    const at = trimmed.indexOf("=");
    if (at < 0) return;
    strings[trimmed.slice(0, at).trim()] = trimmed.slice(at + 1).trim();
  });
  return strings;
}

const strings = stringsFromFtl(ftl);
const lets = app.slice(app.indexOf("let boot = null;"), app.indexOf("const DASH"));
const logic = sliceBetween(app, "function t(", "function bindLan(");

function chromeBin() {
  const listed = [process.env.CHROME_PATH, "google-chrome", "google-chrome-stable", "chromium", "chromium-browser"];
  for (const bin of listed) {
    if (!bin) continue;
    const found = spawnSync(bin, ["--version"], { encoding: "utf8" });
    if (found.status === 0) return bin;
  }
  assert.fail("chrome is required to measure the sync preview");
}

function hub(rows) {
  return {
    mode: "preview",
    code: "",
    message: "",
    peers: [{ name: "Desk", spki: "aa", host: "100.64.0.2", port: 43720 }],
    tailscale: ["100.80.204.119"],
    listen_port: 43720,
    listening: true,
    name: "OnePlus",
    port: 43720,
    preview: { rows },
  };
}

const conflictRows = [
  { id: "rifle", kind: "rifle", status: "new", label: "New rifle" },
  { id: "load", kind: "load", status: "updated", label: "Updated load" },
  { id: "drop", kind: "bullet", status: "deleted", label: "Removed bullet" },
  {
    id: "fight",
    kind: "rifle",
    status: "conflict",
    label: "Shared rifle",
    conflicts: [{ field: "name", local: "Mine", remote: "Theirs" }],
  },
];

function page(rows) {
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"><style>`
    + css
    + `html, body, #app { height: 100%; margin: 0; }
  </style></head><body class="platform-android ho"><div id="app"></div>
  <script type="application/json" id="probe">{}</script>
  <script>` + `
    window.onerror = (message, source, line, col) => {
      const probe = document.getElementById("probe");
      if (probe) probe.textContent = JSON.stringify({ message: String(message), line: line, col: col });
    };
    ` + lets + `
    boot = { strings: ${JSON.stringify(strings)}, platform: "android", settings: { preset: "metric", onboarded: true }, disclaimer: "personal" };
    page = "sync";
    syncTab = "phone";
    lan = ${JSON.stringify(hub(rows))};
    lanChoices = {};
    ` + logic + `
    function box(id) {
      const node = document.getElementById(id);
      if (!node) return null;
      const rect = node.getBoundingClientRect();
      return { top: rect.top, bottom: rect.bottom, left: rect.left, right: rect.right, w: rect.width, h: rect.height, text: node.textContent };
    }
    function run() {
      document.getElementById("app").innerHTML = syncHtml();
      const apply = box("lan-apply");
      const cancel = box("lan-stop");
      const rail = document.querySelector(".ho-rail");
      const main = document.querySelector(".ho-main");
      const railBox = rail ? rail.getBoundingClientRect() : null;
      const mainBox = main ? main.getBoundingClientRect() : null;
      const shell = document.querySelector(".ho-shell");
      const shellStyle = shell ? getComputedStyle(shell) : null;
      document.getElementById("probe").textContent = JSON.stringify({
        apply, cancel,
        stops: document.querySelectorAll("#lan-stop").length,
        applies: document.querySelectorAll("#lan-apply").length,
        innerWidth: window.innerWidth,
        innerHeight: window.innerHeight,
        railLeft: railBox && railBox.left,
        mainLeft: mainBox && mainBox.left,
        bar: !!document.querySelector(".ho-shell > .ho-sync-actions"),
        rows: shellStyle && shellStyle.gridTemplateRows,
        shellH: shell && shell.getBoundingClientRect().height,
      });
    }
    run();
  </script></body></html>`;
}

function pairedPage() {
  const listen = {
    mode: "listen",
    code: "",
    message: "",
    peers: [{ name: "Omawork", spki: "aa", host: "100.80.204.119", port: 43720 }],
    tailscale: ["100.88.250.118"],
    listen_port: 43720,
    listening: true,
    name: "Oneplus13",
    port: 43720,
  };
  const preview = Object.assign({}, listen, { mode: "preview", preview: { rows: conflictRows } });
  const applied = Object.assign({}, listen, { mode: "done", message: "applied" });
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"><style>`
    + css
    + `</style></head><body class="platform-android ho"><div id="app"></div>
  <script type="application/json" id="probe">{}</script>
  <script>` + `
    window.onerror = (message, source, line, col) => {
      const probe = document.getElementById("probe");
      if (probe) probe.textContent = JSON.stringify({ message: String(message), line: line, col: col });
    };
    ` + lets + `
    boot = { strings: ${JSON.stringify(strings)}, platform: "android", settings: { preset: "metric", onboarded: true }, disclaimer: "personal" };
    page = "sync";
    syncTab = "phone";
    lanChoices = {};
    const listen = ${JSON.stringify(listen)};
    const preview = ${JSON.stringify(preview)};
    const applied = ${JSON.stringify(applied)};
    ` + logic + `
    function reach(selector) {
      const node = document.querySelector(selector);
      if (!node) return null;
      const first = node.getBoundingClientRect();
      const inView = first.width >= 8 && first.height >= 8 && first.top >= -1 && first.left >= -1
        && first.bottom <= window.innerHeight + 1 && first.right <= window.innerWidth + 1;
      node.scrollIntoView({ block: "center", inline: "nearest" });
      const rect = node.getBoundingClientRect();
      const cx = rect.left + rect.width / 2;
      const cy = rect.top + rect.height / 2;
      const onScreen = cx >= 0 && cy >= 0 && cx <= window.innerWidth && cy <= window.innerHeight;
      const top = onScreen ? document.elementFromPoint(cx, cy) : null;
      return {
        inView,
        top: rect.top, bottom: rect.bottom, left: rect.left, right: rect.right,
        w: rect.width, h: rect.height, text: node.textContent,
        hit: !!(top && (top === node || node.contains(top))),
      };
    }
    function snap() {
      document.getElementById("app").innerHTML = syncHtml();
      const sync = document.querySelector("[data-lan-use]");
      const stop = document.getElementById("lan-stop");
      const syncTop = sync ? sync.getBoundingClientRect().top : null;
      const stopTop = stop ? stop.getBoundingClientRect().top : null;
      const peers = document.querySelector(".ho-peers");
      return {
        sync: reach("[data-lan-use]"),
        cancel: reach("#lan-stop"),
        apply: reach("#lan-apply"),
        peers: peers ? peers.textContent : "",
        above: syncTop != null && stopTop != null ? syncTop < stopTop : false,
        bar: !!document.querySelector(".ho-shell > .ho-sync-actions"),
      };
    }
    lan = listen;
    const before = snap();
    lan = preview;
    const during = snap();
    lan = applied;
    const after = snap();
    document.getElementById("probe").textContent = JSON.stringify({
      before, during, after,
      innerWidth: window.innerWidth,
      innerHeight: window.innerHeight,
    });
  </script></body></html>`;
}

function within(box, width, height) {
  assert.ok(box, "missing button");
  assert.ok(box.w >= 8 && box.h >= 8, JSON.stringify(box));
  assert.ok(box.top >= -1, JSON.stringify(box));
  assert.ok(box.left >= -1, JSON.stringify(box));
  assert.ok(box.bottom <= height + 1, JSON.stringify({ box, height }));
  assert.ok(box.right <= width + 1, JSON.stringify({ box, width }));
}

function render(html, width, height) {
  const scriptMark = "<script>\n    window.onerror";
  const scriptAt = html.indexOf(scriptMark);
  assert.ok(scriptAt > 0, "page script missing");
  const script = html.slice(scriptAt + "<script>".length, html.lastIndexOf("</script>"));
  const scriptFile = path.join(os.tmpdir(), `holdover-preview-script-${width}.js`);
  fs.writeFileSync(scriptFile, script);
  const check = spawnSync(process.execPath, ["--check", scriptFile], { encoding: "utf8" });
  if (check.status !== 0) assert.fail(check.stderr || check.stdout);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "holdover-preview-"));
  const file = path.join(dir, "page.html");
  fs.writeFileSync(file, html);
  const args = [
    "--headless=new",
    "--disable-gpu",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--hide-scrollbars",
    "--force-device-scale-factor=1",
    `--window-size=${width},${height}`,
    "--user-data-dir=" + path.join(dir, "profile"),
    "--virtual-time-budget=4000",
    "--dump-dom",
    "file://" + file,
  ];
  return new Promise((resolve, reject) => {
    const child = spawn(chromeBin(), args, { stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    let settled = false;
    const finish = (error, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      child.kill("SIGKILL");
      for (let attempt = 0; attempt < 5; attempt += 1) {
        try {
          fs.rmSync(dir, { recursive: true, force: true });
          break;
        } catch (_error) {
          Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 100);
        }
      }
      if (error) reject(error);
      else resolve(value);
    };
    const timer = setTimeout(() => finish(new Error("chrome produced no document\n" + out.slice(-800))), 20000);
    child.stdout.on("data", (buf) => {
      out += buf.toString();
      if (!out.includes("</html>")) return;
      const match = out.match(/id="probe">([\s\S]*?)<\/script>/);
      if (!match) {
        finish(new Error("rendered probe missing\n" + out.slice(0, 500)));
        return;
      }
      const raw = match[1]
        .replace(/&quot;/g, '"')
        .replace(/&#39;/g, "'")
        .replace(/&lt;/g, "<")
        .replace(/&gt;/g, ">")
        .replace(/&amp;/g, "&");
      try {
        finish(null, JSON.parse(raw));
      } catch (error) {
        finish(error);
      }
    });
    child.on("error", (error) => finish(error));
  });
}

test("the preview action bar is outside the scrolling list", () => {
  assert.match(app, /class="ho-sync-actions"/);
  assert.match(app, /has-sync-actions/);
  assert.match(app, /lanPreviewActions\(\)/);
  assert.match(css, /\.ho-shell\.has-sync-actions/);
  assert.match(css, /safe-area-inset-bottom/);
  const frame = sliceBetween(app, "function frame(", "function overlays(");
  const actionsAt = frame.indexOf("${actions}");
  const bodyAt = frame.indexOf("ho-body");
  assert.ok(actionsAt > bodyAt, frame);
});

test("apply sync stays on a phone screen for an empty preview and a conflict", { timeout: 180000 }, async () => {
  const phone = { width: 360, height: 792 };
  const empty = await render(page([]), phone.width, phone.height);
  if (empty.message) assert.fail(JSON.stringify(empty));
  assert.equal(empty.applies, 1);
  assert.equal(empty.stops, 1);
  assert.equal(empty.bar, true, JSON.stringify(empty));
  assert.match(empty.apply.text, /Apply sync/);
  assert.match(empty.cancel.text, /Cancel/);
  within(empty.apply, empty.innerWidth, empty.innerHeight);
  within(empty.cancel, empty.innerWidth, empty.innerHeight);

  const busy = await render(page(conflictRows), phone.width, phone.height);
  if (busy.message) assert.fail(JSON.stringify(busy));
  assert.equal(busy.applies, 1);
  assert.equal(busy.stops, 1);
  within(busy.apply, busy.innerWidth, busy.innerHeight);
  within(busy.cancel, busy.innerWidth, busy.innerHeight);

  const desktop = await render(page(conflictRows), 1280, 800);
  if (desktop.message) assert.fail(JSON.stringify(desktop));
  within(desktop.apply, desktop.innerWidth, desktop.innerHeight);
  within(desktop.cancel, desktop.innerWidth, desktop.innerHeight);
  assert.ok(desktop.mainLeft > desktop.railLeft, JSON.stringify({ rail: desktop.railLeft, main: desktop.mainLeft }));
});

test("sync now stays on a phone screen before a sync and after apply", { timeout: 180000 }, async () => {
  const phone = { width: 360, height: 792 };
  const view = await render(pairedPage(), phone.width, phone.height);
  if (view.message) assert.fail(JSON.stringify(view));
  assert.ok(view.innerWidth <= 759, JSON.stringify({ innerWidth: view.innerWidth, innerHeight: view.innerHeight }));

  function synced(state, label) {
    assert.ok(state.sync, label + " " + JSON.stringify(state));
    assert.match(state.sync.text, /Sync now/);
    assert.match(state.peers, /Paired devices/);
    assert.match(state.peers, /Omawork/);
    assert.equal(state.above, true, label + " " + JSON.stringify(state));
    assert.equal(state.sync.inView, true, label + " " + JSON.stringify(state.sync));
    assert.equal(state.sync.hit, true, label + " " + JSON.stringify(state.sync));
    within(state.sync, view.innerWidth, view.innerHeight);
    assert.equal(state.bar, false, label);
  }

  synced(view.before, "before");
  assert.equal(view.during.bar, true, JSON.stringify(view.during));
  assert.match(view.during.apply.text, /Apply sync/);
  assert.match(view.during.cancel.text, /Cancel/);
  within(view.during.apply, view.innerWidth, view.innerHeight);
  within(view.during.cancel, view.innerWidth, view.innerHeight);
  synced(view.after, "after");
  within(view.before.cancel, view.innerWidth, view.innerHeight);
  assert.equal(view.before.cancel.hit, true, JSON.stringify(view.before.cancel));
});
