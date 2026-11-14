import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  readSettings,
  renderSettingsSection,
  replaceSection,
  writeSettings,
} from "../lib/translate.mjs";

function temporary() {
  const directory = fs.mkdtempSync(
    path.join(os.tmpdir(), "llm-tools-settings-"),
  );
  return {
    directory,
    file: path.join(directory, "rola", "dev", "llm-tools.toml"),
  };
}

const GOOD = {
  baseUrl: "https://api.test/v1",
  model: "test-model",
  apiKey: "test-key",
  temperature: 0.4,
  timeout: 30,
  headers: { "X-Test": "yes" },
};

test("an absent file is the defaults and the path it would have", () => {
  const { directory, file } = temporary();
  try {
    const settings = readSettings([file]);
    assert.equal(settings.file, file);
    assert.equal(settings.exists, false);
    assert.equal(settings.error, null);
    assert.deepEqual(settings.config, {
      baseUrl: "",
      model: "",
      apiKey: "",
      temperature: 0.2,
      timeout: 120,
      headers: {},
    });
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("a save writes the section and reads it back", () => {
  const { directory, file } = temporary();
  try {
    const saved = writeSettings(GOOD, [file]);
    assert.equal(saved.exists, true);
    assert.equal(saved.error, null);
    assert.deepEqual(saved.config, GOOD);

    const text = fs.readFileSync(file, "utf8");
    assert.match(text, /^\[translation\]\n/u);
    assert.match(text, /base_url = "https:\/\/api\.test\/v1"\n/u);
    assert.match(text, /\[translation\.headers\]\nX-Test = "yes"\n/u);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("a save leaves another table and its comment alone", () => {
  const { directory, file } = temporary();
  const original = [
    "# kept",
    "[other]",
    "a = 1",
    "",
    "[translation]",
    'base_url = "old"',
    'model = "old"',
    "",
    "[last]",
    "b = 2",
    "",
  ].join("\n");
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, original);
    writeSettings(GOOD, [file]);
    const text = fs.readFileSync(file, "utf8");
    assert.match(text, /^# kept\n\[other\]\na = 1\n/u);
    assert.match(text, /\[last\]\nb = 2\n/u);
    assert.match(
      text,
      /\[translation\]\nbase_url = "https:\/\/api\.test\/v1"/u,
    );
    assert.doesNotMatch(text, /base_url = "old"/u);
    assert.equal(text.endsWith("\n"), true);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("a save keeps the line endings of the file", () => {
  const { directory, file } = temporary();
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(
      file,
      '[translation]\r\nbase_url = "old"\r\nmodel = "old"\r\n',
    );
    writeSettings(GOOD, [file]);
    const text = fs.readFileSync(file, "utf8");
    assert.equal(text.replace(/\r\n/gu, "").includes("\n"), false);
    assert.equal(readSettings([file]).config.model, "test-model");
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("a value with a quote and a backslash survives the round trip", () => {
  const { directory, file } = temporary();
  try {
    const headers = { "X-Quote": 'a "b" c\\d' };
    const saved = writeSettings({ ...GOOD, headers }, [file]);
    assert.equal(saved.config.headers["X-Quote"], 'a "b" c\\d');
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("a setting the file cannot hold is refused and changes nothing", () => {
  const { directory, file } = temporary();
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, '[translation]\nbase_url = "https://kept/v1"\n');
    const before = fs.readFileSync(file, "utf8");

    for (const [input, message] of [
      [{ ...GOOD, baseUrl: "" }, /base_url/u],
      [{ ...GOOD, baseUrl: "ftp://api.test/v1" }, /base_url/u],
      [{ ...GOOD, model: "" }, /model/u],
      [{ ...GOOD, temperature: 3 }, /temperature/u],
      [{ ...GOOD, temperature: -0.1 }, /temperature/u],
      [{ ...GOOD, timeout: 1.5 }, /timeout/u],
      [{ ...GOOD, timeout: 601 }, /timeout/u],
      [{ ...GOOD, headers: { " ": "x" } }, /header has no name/u],
      [{ ...GOOD, headers: { "X-A": 1 } }, /not text/u],
    ]) {
      assert.throws(() => writeSettings(input, [file]), message);
    }

    assert.equal(fs.readFileSync(file, "utf8"), before);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("a broken section is reported and a save replaces it", () => {
  const { directory, file } = temporary();
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, '[translation]\nbase_url "x"\n');
    const broken = readSettings([file]);
    assert.equal(broken.exists, true);
    assert.match(broken.error, /line 2/u);

    const saved = writeSettings(GOOD, [file]);
    assert.equal(saved.error, null);
    assert.equal(saved.config.model, "test-model");
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("the section text holds the table, its keys and its children", () => {
  assert.deepEqual(
    renderSettingsSection({
      baseUrl: "https://api.test/v1",
      model: "m",
      apiKey: "",
      temperature: null,
      timeout: null,
      headers: {},
    }),
    [
      "[translation]",
      'base_url = "https://api.test/v1"',
      'api_key = ""',
      'model = "m"',
    ],
  );
  assert.deepEqual(
    renderSettingsSection({
      baseUrl: "https://api.test/v1",
      model: "m",
      apiKey: "k",
      temperature: 0,
      timeout: 600,
      headers: { "1 bad key": "v" },
    }).slice(-2),
    ["[translation.headers]", '"1 bad key" = "v"'],
  );
});

test("an empty file takes the section without a leading blank line", () => {
  const text = replaceSection("", "translation", [
    "[translation]",
    'model = "m"',
  ]);
  assert.equal(text, '[translation]\nmodel = "m"\n');
});
