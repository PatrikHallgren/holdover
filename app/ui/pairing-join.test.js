// Pair with this code and Choose photograph have to change the screen.
// On 1.5.0-2 the phone kept the idle introduction: joining stored no message,
// a failed command was replaced by the next poll, and the photograph control
// was only a file input.
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
const css = fs.readFileSync(path.join(here, "holdover-b.css"), "utf8");
const appCss = fs.readFileSync(path.join(here, "app.css"), "utf8");
const ftl = fs.readFileSync(path.join(root, "locales/en.ftl"), "utf8");
const manifest = fs.readFileSync(path.join(root, "app/src-tauri/gen/android/app/src/main/AndroidManifest.xml"), "utf8");
const paths = fs.readFileSync(path.join(root, "app/src-tauri/gen/android/app/src/main/res/xml/file_paths.xml"), "utf8");
const FIXTURE = "holdover://pair?v=fixture";

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
  sliceBetween(app, "function pairingSvg(", "\nfunction preferencesBody("),
].join("\n");

function chromeBin() {
  const listed = [process.env.CHROME_PATH, "google-chrome", "google-chrome-stable", "chromium", "chromium-browser"];
  for (const bin of listed) {
    if (!bin) continue;
    const found = spawnSync(bin, ["--version"], { encoding: "utf8" });
    if (found.status === 0) return bin;
  }
  assert.fail("chrome is required to drive the pairing controls");
}

function page() {
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><style>
    ${appCss}
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
    let lan = { mode: "idle", code: "", message: "", peers: [], found: [], name: "Oneplus13" };
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
    window.__joined = "";
    window.__hub = { mode: "idle", code: "", message: "", peers: [], found: [], name: "Oneplus13" };
    window.__act = { mode: "joining", message: "reaching", code: "", peers: [], found: [], name: "Oneplus13" };
    window.__camera = { ok: true, kind: "android" };
    window.__decoded = ${JSON.stringify(FIXTURE)};
    window.__decodeThrow = false;
    function reply(command, args) {
      if (command === "phone_prepare") return window.__camera;
      if (command === "lan_status") return window.__hub;
      const action = args && args.body && args.body.action;
      if (command === "lan_act" && action === "decode-qr") {
        if (window.__decodeThrow) throw new Error("photo-failed");
        return { code: window.__decoded };
      }
      if (command === "lan_act" && action === "pair-join") {
        window.__joined = args.body.code;
        return window.__act;
      }
      return window.__act;
    }
    async function call(command, args) {
      sent.push({ command, action: args && args.body && args.body.action, code: args && args.body && args.body.code });
      const job = Promise.resolve().then(() => reply(command, args));
      jobs.push(job);
      return job;
    }
    async function settle() {
      for (let i = 0; i < 6; i += 1) {
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
    function readFileBase64() {
      return Promise.resolve("AAAA");
    }
    function statusIn(root) {
      const node = root.querySelector("[data-lan-status]");
      if (!node) return null;
      const box = node.getBoundingClientRect();
      return { role: node.getAttribute("role"), text: node.textContent, w: box.width, h: box.height, y: box.y };
    }
    window.setInterval = () => 1;
    async function run() {
      render();
      const code = document.getElementById("lan-code");
      code.value = ${JSON.stringify(FIXTURE)};
      document.getElementById("lan-join").click();
      await settle();
      const reaching = {
        rail: statusIn(document.getElementById("inputs")),
        main: statusIn(document.getElementById("main")),
        joined: window.__joined,
        field: document.getElementById("lan-code").value
      };
      window.__hub = { mode: "idle", code: "pair-unreachable", message: "", peers: [], found: [], name: "Oneplus13" };
      lanStamp = "";
      await pollLan();
      await pollLan();
      const failed = {
        rail: statusIn(document.getElementById("inputs")),
        main: statusIn(document.getElementById("main")),
        intro: document.getElementById("main").textContent
      };
      const gallery = document.getElementById("lan-photo");
      let galleryClicks = 0;
      gallery.click = () => { galleryClicks += 1; };
      document.getElementById("lan-photo-open").click();
      const opened = galleryClicks;
      const transfer = new DataTransfer();
      transfer.items.add(new File([new Uint8Array([1, 2, 3])], "code.png", { type: "image/png" }));
      gallery.files = transfer.files;
      gallery.dispatchEvent(new Event("change"));
      await settle();
      const read = {
        field: document.getElementById("lan-code").value,
        main: statusIn(document.getElementById("main")),
        files: document.getElementById("lan-photo").files.length,
        label: document.getElementById("lan-photo").value
      };
      window.__hub = { mode: "idle", code: "", message: "", peers: [], found: [], name: "Oneplus13" };
      lanStamp = "idle-after-photo";
      await pollLan();
      const heldReady = statusIn(document.getElementById("main"));
      window.__decodeThrow = true;
      const again = document.getElementById("lan-photo");
      again.click = () => {};
      const bad = new DataTransfer();
      bad.items.add(new File([new Uint8Array([9])], "nope.png", { type: "image/png" }));
      again.files = bad.files;
      again.dispatchEvent(new Event("change"));
      await settle();
      const broken = statusIn(document.getElementById("main"));
      window.__hub = { mode: "idle", code: "", message: "", peers: [], found: [], name: "Oneplus13" };
      lanStamp = "idle-after-broken";
      await pollLan();
      const heldBroken = statusIn(document.getElementById("main"));
      window.__camera = { ok: false, kind: "android" };
      const cameraNow = document.getElementById("lan-photo-capture");
      let cameraClicks = 0;
      cameraNow.click = () => { cameraClicks += 1; };
      document.getElementById("lan-photo-camera").click();
      await settle();
      const cameraFail = { clicks: cameraClicks, main: statusIn(document.getElementById("main")) };
      const empty = document.getElementById("lan-photo");
      empty.click = () => {};
      empty.files = new DataTransfer().files;
      empty.dispatchEvent(new Event("change"));
      const emptyNote = statusIn(document.getElementById("main"));
      document.getElementById("probe").textContent = JSON.stringify({
        reaching, failed, opened, read, heldReady, broken, heldBroken, cameraFail, emptyNote,
        choose: !!document.getElementById("lan-photo-open"),
        take: !!document.getElementById("lan-photo-camera")
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
  const scriptFile = path.join(os.tmpdir(), "holdover-join-script.js");
  fs.writeFileSync(scriptFile, script);
  const check = spawnSync(process.execPath, ["--check", scriptFile], { encoding: "utf8" });
  if (check.status !== 0) assert.fail(check.stderr || check.stdout);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "holdover-join-"));
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

test("the phone manifest can see a photograph picker and the camera", () => {
  assert.match(app, /id="lan-photo-open"/);
  assert.match(app, /id="lan-photo-camera"/);
  assert.match(app, /lanLiveCode\(node \? node\.value : lanCode\)/);
  assert.doesNotMatch(app, /click\("lan-join", \(\) => lanAct\(\{ action: "pair-join", code: lanCode \}\)\)/);
  assert.match(css, /\.ho-file-hidden \{[^}]*clip:/);
  assert.doesNotMatch(css, /\.ho-file-hidden \{[^}]*display:\s*none/);
  assert.match(manifest, /android.intent.action.GET_CONTENT/);
  assert.match(manifest, /android.intent.action.OPEN_DOCUMENT/);
  assert.match(manifest, /android.intent.action.PICK/);
  assert.match(manifest, /android.media.action.IMAGE_CAPTURE/);
  assert.match(paths, /external-files-path/);
  assert.match(ftl, /lan-reaching = /);
  assert.match(ftl, /lan-pair-unreachable = /);
  assert.match(ftl, /lan-photo-failed = /);
});

test("pairing and photograph controls change the screen and keep the result", { timeout: 180000 }, async () => {
  const report = await render(page());
  if (!report.reaching) assert.fail(JSON.stringify(report).slice(0, 2000));
  assert.equal(report.reaching.joined, FIXTURE);
  assert.equal(report.reaching.field, FIXTURE);
  assert.equal(report.reaching.rail.role, "status");
  assert.match(report.reaching.rail.text, /Reaching the other Holdover/);
  assert.equal(report.reaching.main.role, "status");
  assert.ok(report.reaching.main.h >= 8, JSON.stringify(report.reaching.main));
  assert.ok(report.reaching.main.w >= 40, JSON.stringify(report.reaching.main));
  assert.equal(report.failed.main.role, "alert");
  assert.match(report.failed.main.text, /Could not reach the other Holdover/);
  assert.ok(report.failed.main.h >= 8, JSON.stringify(report.failed));
  assert.match(report.failed.intro, /Could not reach the other Holdover/);
  assert.equal(report.opened, 1);
  assert.equal(report.choose, true);
  assert.equal(report.take, true);
  assert.equal(report.read.field, FIXTURE);
  assert.equal(report.read.files, 0);
  assert.match(report.read.main.text, /Press Pair with this code/);
  assert.match(report.heldReady.text, /Press Pair with this code/);
  assert.equal(report.broken.role, "alert");
  assert.match(report.broken.text, /could not be read/);
  assert.match(report.heldBroken.text, /could not be read/);
  assert.equal(report.cameraFail.clicks, 0);
  assert.match(report.cameraFail.main.text, /could not be read/);
  assert.match(report.emptyNote.text, /No photograph was chosen/);
});
