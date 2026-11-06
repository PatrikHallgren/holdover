// Renders the Sync > Phone pairing picture from the functions in app.js.
// The installed 1.5.0-1 window dropped the picture because the SVG from
// qrcode starts with an XML declaration, and the rail only kept the text.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const app = fs.readFileSync(path.join(here, "app.js"), "utf8");
const css = fs.readFileSync(path.join(here, "holdover-b.css"), "utf8");
const appCss = fs.readFileSync(path.join(here, "app.css"), "utf8");
const ftl = fs.readFileSync(path.join(here, "../../locales/en.ftl"), "utf8");

const SVG = '<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="220" height="220" viewBox="0 0 220 220" shape-rendering="crispEdges"><rect x="0" y="0" width="220" height="220" fill="#fff"/><path fill="#000" d="M20 20h40v40H20V20"/></svg>';
const RAW = '<?xml version="1.0" standalone="yes"?>' + SVG;
const TEXT = "holdover://pair?v=fixture";
const NOW = 1_700_000_000;

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

function load(state) {
  const body = sliceBetween(app, "function pairingSvg(", "\nfunction bindLan(");
  const source = `
    const boot = state.boot;
    const lan = state.lan;
    const lanName = state.lanName;
    const lanCode = state.lanCode;
    const lanHost = state.lanHost;
    const lanPort = state.lanPort;
    const lanInclude = state.lanInclude;
    const lanChoices = state.lanChoices;
    ${sliceBetween(app, "function t(", "function touchUi(")}
    ${sliceBetween(app, "function tf(", "function shownUnit(")}
    ${sliceBetween(app, "function esc(", "async function call(")}
    ${body}
    return { pairingSvg, lanPairCard, lanRail, lanMain, lanExpiryLine };
  `;
  return new Function("state", source)(state);
}

function view(extra) {
  return Object.assign({
    mode: "pair",
    offer_url: TEXT,
    offer_svg: RAW,
    expires: NOW + 90,
    code: "",
    peers: [],
    name: "Omawork",
  }, extra);
}

function stateFor(current) {
  return {
    boot: { strings },
    lan: current,
    lanName: "Omawork",
    lanCode: "",
    lanHost: "",
    lanPort: "",
    lanInclude: false,
    lanChoices: {},
  };
}

function chromeBin() {
  const listed = [process.env.CHROME_PATH, "google-chrome", "google-chrome-stable", "chromium", "chromium-browser"];
  for (const bin of listed) {
    if (!bin) continue;
    const found = spawnSync(bin, ["--version"], { encoding: "utf8" });
    if (found.status === 0) return bin;
  }
  assert.fail("chrome is required to render the pairing picture");
}

function page(rail, main, large) {
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><style>
    ${appCss}
    ${css}
    html, body, .ho { height: 100%; margin: 0; }
  </style></head><body class="${large ? "large-text" : ""}"><div class="ho"><div class="ho-shell"><div class="ho-body with-list"><aside class="ho-rail" id="inputs">${rail}</aside><main class="ho-main" id="main">${main}</main></div></div></div>
  <script type="application/json" id="probe">{}</script>
  <script>
    const svg = document.querySelector("main .ho-qr svg");
    const railSvg = document.querySelector(".ho-rail svg");
    const pair = document.querySelector(".ho-pair");
    const rail = document.querySelector(".ho-rail");
    const text = document.getElementById("lan-offer-text");
    const alert = document.querySelector("main [role=alert]");
    const expiry = document.getElementById("lan-expiry");
    const pad = document.querySelector(".ho-qr");
    let sample = null;
    if (svg) {
      const w = Number(svg.getAttribute("width"));
      const h = Number(svg.getAttribute("height"));
      const canvas = document.createElement("canvas");
      canvas.width = w;
      canvas.height = h;
      const ctx = canvas.getContext("2d");
      ctx.imageSmoothingEnabled = false;
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(0, 0, w, h);
      const path = svg.querySelector("path");
      ctx.fillStyle = path.getAttribute("fill") || "#000000";
      ctx.fill(new Path2D(path.getAttribute("d")));
      const corner = ctx.getImageData(1, 1, 1, 1).data;
      const modulePixel = ctx.getImageData(30, 30, 1, 1).data;
      const box = svg.getBoundingClientRect();
      sample = {
        corner: [corner[0], corner[1], corner[2]],
        module: [modulePixel[0], modulePixel[1], modulePixel[2]],
        fill: path.getAttribute("fill"),
        bg: svg.querySelector("rect").getAttribute("fill"),
        width: box.width,
        height: box.height,
        x: box.x,
        y: box.y
      };
    }
    const railBox = rail.getBoundingClientRect();
    const pairBox = pair ? pair.getBoundingClientRect() : null;
    const textBox = text ? text.getBoundingClientRect() : null;
    const alertBox = alert ? alert.getBoundingClientRect() : null;
    document.getElementById("probe").textContent = JSON.stringify({
      innerWidth: window.innerWidth,
      innerHeight: window.innerHeight,
      dpr: window.devicePixelRatio,
      railSvg: !!railSvg,
      mainHasSvg: !!document.querySelector("main svg"),
      sample,
      pad: pad ? getComputedStyle(pad).backgroundColor : "",
      rail: { x: railBox.x, y: railBox.y, w: railBox.width, h: railBox.height },
      pair: pairBox && { x: pairBox.x, y: pairBox.y, w: pairBox.width, h: pairBox.height },
      text: textBox && { x: textBox.x, y: textBox.y, w: textBox.width, h: textBox.height },
      textValue: text ? text.value : "",
      alert: alertBox && { x: alertBox.x, y: alertBox.y, w: alertBox.width, h: alertBox.height, text: alert.textContent },
      expiry: expiry ? expiry.textContent : "",
      railText: rail.textContent
    });
  </script></body></html>`;
}

function render(html, options) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "holdover-pair-"));
  const file = path.join(dir, "page.html");
  fs.writeFileSync(file, html);
  const shot = path.join(dir, "shot.png");
  const args = [
    "--headless=new",
    "--disable-gpu",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--hide-scrollbars",
    "--force-device-scale-factor=" + options.scale,
    "--window-size=" + options.width + "," + options.height,
    "--user-data-dir=" + path.join(dir, "profile"),
    "--screenshot=" + shot,
    "--virtual-time-budget=2000",
    "--dump-dom",
    "file://" + file,
  ];
  const child = spawn(chromeBin(), args, { stdio: ["ignore", "pipe", "pipe"] });
  let out = "";
  let settled = false;
  const done = new Promise((resolve, reject) => {
    const finish = (error, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      child.kill("SIGKILL");
      if (options.shot && fs.existsSync(shot)) fs.copyFileSync(shot, options.shot);
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
    const timer = setTimeout(() => finish(new Error("chrome produced no document")), 20000);
    child.stdout.on("data", (buf) => {
      out += buf.toString();
      if (!out.includes("</html>")) return;
      const match = out.match(/id="probe">([\s\S]*?)<\/script>/);
      if (!match) {
        finish(new Error("rendered probe missing"));
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
  return done;
}

test("an xml-prefixed picture is drawn in the main pane, with text and expiry", () => {
  assert.equal(RAW.trim().startsWith("<svg"), false);
  assert.match(app, /phoneOn \? lanMain\(\)/);
  assert.match(app, /lanPairCard\(lan \|\| \{\}, lanNow\(\)\)/);
  assert.doesNotMatch(app, /startsWith\("<svg"\)/);
  assert.match(css, /\.ho-qr svg \{[^}]*width:\s*100%/);
  assert.doesNotMatch(css, /\.ho-qr svg \{[^}]*width:\s*220px/);
  assert.match(css, /\.ho-list \[aria-selected="true"\] \.ho-badge[^{]*\{[^}]*color:\s*var\(--text\)/);
  assert.match(css, /ho-pair \{[^}]*order:\s*-4/);

  const api = load(stateFor(view()));
  assert.equal(api.pairingSvg(RAW).startsWith("<svg"), true);
  assert.equal(api.pairingSvg('<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>'), "");
  const card = api.lanPairCard(view(), NOW);
  assert.match(card, /<section class="ho-pair"/);
  assert.match(card, /<svg /);
  assert.doesNotMatch(card, /<\?xml/);
  assert.match(card, /id="lan-expiry"/);
  assert.match(card, /2 minutes/);
  assert.match(card, /id="lan-offer-text"/);
  assert.match(api.lanExpiryLine(NOW + 30, NOW), /1 minute /);
  assert.doesNotMatch(api.lanExpiryLine(NOW + 30, NOW), /1 minutes/);
  const rail = api.lanRail();
  assert.doesNotMatch(rail, /<svg/);
  assert.doesNotMatch(rail, /holdover:\/\/pair/);
  assert.match(rail, /main area/);

  const evil = api.lanPairCard(view({ offer_svg: '<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>' }), NOW);
  assert.doesNotMatch(evil, /<script/i);
  assert.doesNotMatch(evil, /<svg/i);
  assert.match(evil, /could not be drawn/);
  assert.match(evil, /id="lan-offer-text"/);

  const expired = api.lanPairCard(view({ code: "pair-expired", expires: NOW - 5 }), NOW);
  assert.doesNotMatch(expired, /<svg/);
  assert.doesNotMatch(expired, /lan-offer-text/);
  assert.match(expired, /expired/);

  const stale = api.lanPairCard(view({ expires: NOW - 1 }), NOW);
  assert.doesNotMatch(stale, /<svg/);

  const refused = api.lanPairCard(view({ code: "pair-pin" }), NOW);
  assert.match(refused, /<svg /);
  assert.match(refused, /not the one in the code/);

  const live = view({ expires: Math.floor(Date.now() / 1000) + 600 });
  const main = load(stateFor(live)).lanMain();
  assert.match(main, /<section class="ho-pair"/);
  assert.match(main, /<svg /);
  assert.doesNotMatch(main, /<\?xml/);
});

test("the picture stays large on a desktop, a scaled desktop, and a narrow window", { timeout: 180000 }, async () => {
  const api = load(stateFor(view()));
  const html = page(api.lanRail(), api.lanPairCard(view(), NOW), false);
  const desktop = await render(html, { width: 1280, height: 800, scale: 1, shot: "/tmp/holdover-pair-desktop.png" });
  assert.equal(desktop.railSvg, false);
  assert.equal(desktop.mainHasSvg, true);
  assert.equal(desktop.alert, null);
  assert.equal(desktop.textValue, TEXT);
  assert.equal(desktop.sample.fill, "#000");
  assert.equal(desktop.sample.bg, "#fff");
  assert.equal(desktop.pad, "rgb(255, 255, 255)");
  assert.ok(desktop.sample.corner[0] >= 250 && desktop.sample.module[0] <= 5, JSON.stringify(desktop.sample));
  assert.ok(desktop.sample.width >= 400, JSON.stringify({ sample: desktop.sample, inner: desktop.innerWidth, rail: desktop.rail }));
  assert.ok(desktop.sample.height >= 400);
  assert.ok(Math.abs(desktop.sample.width - desktop.sample.height) < 3);
  assert.ok(desktop.sample.x > desktop.rail.x + desktop.rail.w - 2, JSON.stringify({ sample: desktop.sample, rail: desktop.rail }));
  assert.ok(desktop.text.w <= desktop.pair.w + 2);
  assert.ok(desktop.text.y > desktop.sample.y);
  assert.equal(desktop.railText.includes(TEXT), false);
  assert.match(desktop.expiry, /2 minutes/);

  const scaled = await render(html, { width: 1280, height: 800, scale: 1.5 });
  assert.ok(Math.abs(scaled.dpr - 1.5) < 0.05, String(scaled.dpr));
  assert.ok(Math.abs(scaled.sample.width - desktop.sample.width) < 8, JSON.stringify({ scaled: scaled.sample.width, desktop: desktop.sample.width }));
  assert.ok(Math.abs(scaled.sample.width - scaled.sample.height) < 3);

  const large = await render(page(api.lanRail(), api.lanPairCard(view(), NOW), true), { width: 1280, height: 800, scale: 1 });
  assert.ok(Math.abs(large.sample.width - desktop.sample.width) < 8, JSON.stringify({ large: large.sample.width, desktop: desktop.sample.width }));

  const narrow = await render(html, { width: 390, height: 844, scale: 2, shot: "/tmp/holdover-pair-narrow.png" });
  assert.equal(narrow.railSvg, false);
  assert.equal(narrow.mainHasSvg, true);
  assert.equal(narrow.textValue, TEXT);
  assert.ok(narrow.sample.width >= 240, JSON.stringify({ sample: narrow.sample, inner: narrow.innerWidth, pair: narrow.pair }));
  assert.ok(Math.abs(narrow.sample.width - narrow.sample.height) < 3);
  assert.ok(narrow.pair.y <= narrow.rail.y + 1, JSON.stringify({ pair: narrow.pair, rail: narrow.rail, inner: narrow.innerWidth }));
  assert.ok(narrow.text.w <= narrow.pair.w + 2);
  assert.equal(narrow.sample.module[0] <= 5 && narrow.sample.corner[0] >= 250, true);
  assert.equal(narrow.pad, "rgb(255, 255, 255)");

  const expiredApi = load(stateFor(view({ code: "pair-expired", expires: NOW - 5 })));
  const expired = await render(page(expiredApi.lanRail(), expiredApi.lanPairCard(view({ code: "pair-expired", expires: NOW - 5 }), NOW), false), { width: 1280, height: 800, scale: 1 });
  assert.equal(expired.mainHasSvg, false);
  assert.equal(expired.sample, null);
  assert.match(expired.alert.text, /expired/);
  assert.ok(expired.alert.h >= 8);
  assert.ok(expired.alert.x > expired.rail.x + expired.rail.w - 2);

  const bad = view({ offer_svg: "not-a-picture" });
  const badApi = load(stateFor(bad));
  const failed = await render(page(badApi.lanRail(), badApi.lanPairCard(bad, NOW), false), { width: 1280, height: 800, scale: 1 });
  assert.equal(failed.mainHasSvg, false);
  assert.match(failed.alert.text, /could not be drawn/);
  assert.equal(failed.textValue, TEXT);
  assert.ok(failed.alert.x > failed.rail.x + failed.rail.w - 2);
  assert.ok(failed.text.x > failed.rail.x + failed.rail.w - 2);
});
