// Start sync has to say it is reaching, and a failure has to stay after the
// next status poll. On 1.5.2 the saved port was stale, the port field was
// empty, and a failed command was replaced by the idle introduction.
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
const css = fs.readFileSync(path.join(here, "app.css"), "utf8");
const ftl = fs.readFileSync(path.join(root, "locales/en.ftl"), "utf8");
const bridge = fs.readFileSync(
  path.join(root, "app/src-tauri/gen/android/app/src/main/java/se/hallgren/holdover/PhoneBridge.kt"),
  "utf8",
);

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
const logic = [
  sliceBetween(app, "function t(", "function touchUi("),
  sliceBetween(app, "function tf(", "function shownUnit("),
  sliceBetween(app, "function esc(", "async function call("),
  sliceBetween(app, "function pairingSvg(", "function preferencesBody("),
].join("\n");

function chromeBin() {
  const listed = [process.env.CHROME_PATH, "google-chrome", "google-chrome-stable", "chromium", "chromium-browser"];
  for (const bin of listed) {
    if (!bin) continue;
    const found = spawnSync(bin, ["--version"], { encoding: "utf8" });
    if (found.status === 0) return bin;
  }
  assert.fail("chrome is required to drive the sync controls");
}

const hub = {
  mode: "idle",
  code: "",
  message: "",
  peers: [{ name: "Omawork", spki: "aa", host: "192.168.1.20,100.64.0.2", port: 34351 }],
  found: [],
  tailscale: ["100.64.0.1"],
  listen_port: 43720,
  listening: true,
  name: "Oneplus13",
  port: 0,
};

function page() {
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><style>
    ${css}
    html, body, .ho { height: auto; margin: 0; }
  </style></head><body class="platform-android"><div class="ho"><div class="ho-shell"><div class="ho-body with-list"><aside class="ho-rail" id="inputs"></aside><main class="ho-main" id="main"></main></div></div></div>
  <script type="application/json" id="probe">{}</script>
  <script>` + `
    window.onerror = (message, source, line, col) => {
      const probe = document.getElementById("probe");
      if (probe) probe.textContent = JSON.stringify({ message: String(message), line: line, col: col });
    };
    ` + `
    const boot = { strings: ${JSON.stringify(strings)}, platform: "android" };
    let page = "sync";
    let syncTab = "phone";
    let lan = ${JSON.stringify(hub)};
    let lanStamp = "";
    let lanHold = null;
    let lanCode = "";
    let lanHost = "";
    let lanPort = "";
    let lanName = "Oneplus13";
    let lanInclude = false;
    let lanChoices = {};
    let lanTimer = 1;
    let selectedId = "";
    const sent = [];
    const jobs = [];
    window.__hub = ${JSON.stringify(hub)};
    function reply(command, args) {
      const action = args && args.body && args.body.action;
      if (command === "lan_act" && action === "sync-begin") {
        throw new Error("lan-sync-unreachable");
      }
      if (command === "lan_status") return window.__hub;
      return window.__hub;
    }
    async function call(command, args) {
      sent.push({
        command,
        action: args && args.body && args.body.action,
        host: args && args.body && args.body.host,
        port: args && args.body && args.body.port,
        spki: args && args.body && args.body.spki,
      });
      const job = Promise.resolve().then(() => reply(command, args));
      jobs.push(job);
      return job;
    }
    async function settle() {
      for (let i = 0; i < 8; i += 1) {
        const batch = jobs.splice(0, jobs.length);
        if (!batch.length) {
          await Promise.resolve();
          continue;
        }
        await Promise.allSettled(batch);
      }
    }
    function render() {
      document.getElementById("inputs").innerHTML = lanRail();
      document.getElementById("main").innerHTML = lanMain();
      bindLan();
    }
    ` + logic + `
    function statusIn(root) {
      const node = root.querySelector("[data-lan-status]");
      if (!node) return null;
      const box = node.getBoundingClientRect();
      return { role: node.getAttribute("role"), text: node.textContent, w: box.width, h: box.height };
    }
    window.setInterval = () => 1;
    async function run() {
      render();
      const self = document.querySelector("[data-lan-self]");
      const shown = {
        text: self ? self.textContent : "",
        readyButton: !!document.getElementById("lan-listen"),
        findButton: !!document.getElementById("lan-discover"),
        portField: !!document.getElementById("lan-port"),
      };
      document.querySelector("[data-lan-use]").click();
      const reaching = statusIn(document.getElementById("main"));
      await settle();
      const failed = {
        rail: statusIn(document.getElementById("inputs")),
        main: statusIn(document.getElementById("main")),
      };
      window.__hub = Object.assign({}, window.__hub, { mode: "idle", code: "", message: "" });
      lanStamp = "";
      await pollLan();
      await pollLan();
      const held = {
        rail: statusIn(document.getElementById("inputs")),
        main: statusIn(document.getElementById("main")),
      };
      const sync = sent.find((row) => row.action === "sync-begin");
      document.getElementById("probe").textContent = JSON.stringify({
        shown, reaching, failed, held, sync,
      });
    }
    run().catch((error) => {
      const probe = document.getElementById("probe");
      if (probe) probe.textContent = JSON.stringify({ stage: "run", error: String(error), stack: String(error && error.stack) });
    });
  </script></body></html>`;
}

function decodeProbe(raw) {
  return JSON.parse(raw
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&"));
}

function render(html) {
  const scriptMark = "<script>\n    window.onerror";
  const scriptAt = html.indexOf(scriptMark);
  assert.ok(scriptAt > 0, "page script missing");
  const script = html.slice(scriptAt + "<script>".length, html.lastIndexOf("</script>"));
  const scriptFile = path.join(os.tmpdir(), "holdover-sync-script.js");
  fs.writeFileSync(scriptFile, script);
  const check = spawnSync(process.execPath, ["--check", scriptFile], { encoding: "utf8" });
  if (check.status !== 0) assert.fail(check.stderr || check.stdout);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "holdover-sync-"));
  const file = path.join(dir, "page.html");
  fs.writeFileSync(file, html);
  const args = [
    "--headless=new",
    "--disable-gpu",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--hide-scrollbars",
    "--force-device-scale-factor=2",
    "--window-size=390,844",
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
    const timer = setTimeout(() => finish(new Error("chrome produced no document\n" + out.slice(-500))), 20000);
    child.stdout.on("data", (buf) => {
      out += buf.toString();
      if (!out.includes("</html>")) return;
      const match = out.match(/id="probe">([\s\S]*?)<\/script>/);
      if (!match) {
        finish(new Error("rendered probe missing\n" + out.slice(0, 400)));
        return;
      }
      try {
        finish(null, decodeProbe(match[1]));
      } catch (error) {
        finish(error);
      }
    });
    child.on("error", (error) => finish(error));
  });
}

test("sync status strings name the port, the miss, and an empty search", () => {
  assert.match(app, /data-lan-self/);
  assert.match(app, /tailscale/);
  assert.match(app, /function lanErrorCode/);
  assert.match(ftl, /lan-sync-unreachable = /);
  assert.match(ftl, /lan-ready = Sync ready over Tailscale/);
  assert.match(ftl, /lan-need-tailscale = /);
  assert.doesNotMatch(app, /lan-firewall/);
  assert.doesNotMatch(ftl, /ufw allow/);
  assert.match(app, /view\.detail/);
  const messages = stringsFromFtl(ftl);
  const t = (key) => messages[key] || key;
  const lanMessageFor = new Function(
    "t",
    `${sliceBetween(app, "function lanMessageFor(", "function lanMessage(")}\nreturn lanMessageFor;`,
  )(t);
  const shown = lanMessageFor({
    code: "sync-unreachable",
    detail: "tried 100.64.0.2:43720 refused",
  });
  assert.match(shown, /100\.64\.0\.2:43720 refused/);
  assert.match(shown, /Could not reach that Holdover over Tailscale/);
  assert.match(ftl, /lan-sync-reaching = /);
  assert.match(bridge, /MulticastLock/);
  assert.match(bridge, /fun discoverMdns/);
  assert.match(bridge, /fun advertiseMdns/);
  assert.match(bridge, /NsdManager/);
});

test("sync now dials port 43720 and the error stays on screen", { timeout: 180000 }, async () => {
  const report = await render(page());
  if (!report.shown) assert.fail(JSON.stringify(report).slice(0, 2000));
  assert.match(report.shown.text, /Sync ready over Tailscale at Oneplus13\/100\.64\.0\.1:43720/);
  assert.equal(report.shown.readyButton, false);
  assert.equal(report.shown.findButton, false);
  assert.equal(report.shown.portField, false);
  assert.equal(report.reaching.role, "status");
  assert.match(report.reaching.text, /Reaching the paired Holdover over Tailscale/);
  assert.equal(report.sync.action, "sync-begin");
  assert.equal(report.sync.port, 43720);
  assert.equal(report.sync.host, "");
  assert.equal(report.sync.spki, "aa");
  assert.equal(report.failed.main.role, "alert");
  assert.match(report.failed.main.text, /Could not reach that Holdover over Tailscale/);
  assert.ok(report.failed.main.h >= 8, JSON.stringify(report.failed.main));
  assert.match(report.held.main.text, /Could not reach that Holdover over Tailscale/);
  assert.match(report.held.rail.text, /Could not reach that Holdover over Tailscale/);
});
