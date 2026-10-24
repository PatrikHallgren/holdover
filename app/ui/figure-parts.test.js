// Loads figureParts from app.js. A copied function would not catch a regression
// in the file the window actually runs.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const app = fs.readFileSync(path.join(here, "app.js"), "utf8");

function loadFigures(source) {
  const start = source.indexOf("function figureParts(");
  const end = source.indexOf("\nfunction dirGlyph", start);
  assert.ok(start >= 0 && end > start, "figureParts is in app.js");
  const body = source.slice(start, end);
  return new Function(`${body}\nreturn { figureParts, figureShown };`)();
}

const { figureParts, figureShown } = loadFigures(app);

test("hold figures keep every digit at 10 and above", () => {
  assert.deepEqual(figureParts({ primary: "17" }), { sign: "", num: "17", unit: "" });
  assert.deepEqual(figureParts({ primary: "54" }), { sign: "", num: "54", unit: "" });
  assert.deepEqual(figureParts({ primary: "12.35 MOA" }), { sign: "", num: "12.35", unit: "MOA" });
  assert.deepEqual(figureParts({ primary: "10.2 mrad" }), { sign: "", num: "10.2", unit: "mrad" });
  assert.deepEqual(figureParts({ primary: "18.6138 MOA" }), { sign: "", num: "18.6138", unit: "MOA" });
  assert.deepEqual(figureParts({ primary: "54 clicks" }), { sign: "", num: "54", unit: "clicks" });
});

test("negative holds keep the digits and the sign", () => {
  assert.deepEqual(figureParts({ primary: "-17.5 mrad" }), { sign: "-", num: "17.5", unit: "mrad" });
  assert.deepEqual(figureParts({ primary: "−8.25 mrad" }), { sign: "−", num: "8.25", unit: "mrad" });
  assert.deepEqual(figureParts({ primary: "-54" }), { sign: "-", num: "54", unit: "" });
  assert.deepEqual(figureParts({ primary: "+12.35 MOA" }), { sign: "+", num: "12.35", unit: "MOA" });
});

test("the big figure rounds and keeps a click count whole", () => {
  assert.equal(figureShown({ primary: "17" }).num, "17");
  assert.equal(figureShown({ primary: "54" }).num, "54");
  assert.equal(figureShown({ primary: "54 clicks" }).num, "54");
  assert.equal(figureShown({ primary: "12.35 MOA" }).num, "12.35");
  assert.equal(figureShown({ primary: "10.2 mrad" }).num, "10.20");
  assert.equal(figureShown({ primary: "18.6138 MOA" }).num, "18.61");
  assert.equal(figureShown({ primary: "12.36 cm/100 m" }).num, "12.4");
  assert.equal(figureShown({ primary: "-17.5 mrad" }).num, "17.50");
  assert.equal(figureParts({ primary: "-54" }).sign, "-");
});

test("a missing hold stays a dash and is not parsed as a number", () => {
  assert.deepEqual(figureParts(null), { sign: "", num: "—", unit: "" });
  assert.deepEqual(figureParts({ primary: "no scope" }), { sign: "", num: "no scope", unit: "" });
});

function characterClasses(source) {
  const found = [];
  for (let i = 0; i < source.length; i += 1) {
    if (source[i] !== "[" || source[i + 1] === "[") continue;
    let j = i + 1;
    let body = "";
    while (j < source.length && source[j] !== "]" && source[j] !== "\n") {
      if (source[j] === "\\") {
        body += source[j] + (source[j + 1] || "");
        j += 2;
        continue;
      }
      body += source[j];
      j += 1;
    }
    if (source[j] === "]") found.push(body);
    i = j;
  }
  return found;
}

function hyphenIsRange(body) {
  // An unescaped hyphen is literal at either end of a class. Between "+" and
  // "−" it is a range that includes the digits. An escaped "\-" is literal.
  const tokens = [];
  for (let i = 0; i < body.length; i += 1) {
    if (body[i] === "\\") {
      if (body[i + 1] === "u" && /^[0-9a-fA-F]{4}/.test(body.slice(i + 2, i + 6))) {
        const code = Number.parseInt(body.slice(i + 2, i + 6), 16);
        tokens.push(code === 0x2212 ? "minus" : "other");
        i += 5;
        continue;
      }
      tokens.push("other");
      i += 1;
      continue;
    }
    if (body[i] === "+") tokens.push("plus");
    else if (body[i] === "−") tokens.push("minus");
    else if (body[i] === "-") tokens.push("hyphen");
    else tokens.push("other");
  }
  const at = tokens.indexOf("hyphen");
  if (at <= 0 || at === tokens.length - 1) return false;
  return tokens.includes("plus") && tokens.includes("minus");
}

test("no other UI character class uses an unescaped +-− range", () => {
  const files = fs.readdirSync(here).filter((name) => name.endsWith(".js"));
  const bad = [];
  files.forEach((name) => {
    const source = fs.readFileSync(path.join(here, name), "utf8");
    characterClasses(source).forEach((body) => {
      if (hyphenIsRange(body)) bad.push(`${name}: [${body}]`);
    });
  });
  assert.deepEqual(bad, []);
  assert.match(app, /\[\+\\-\\u2212\]/);
});
