import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  configCandidates,
  endpoint,
  parseToml,
  readTranslationConfig,
} from "../lib/translate.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));

test("the example is the schema the editor reads", () => {
  const text = fs.readFileSync(
    path.join(HERE, "..", "llm-tools.example.toml"),
    "utf8",
  );
  const parsed = parseToml(text);
  assert.equal(parsed.translation.base_url, "https://api.openai.com/v1");
  assert.equal(parsed.translation.api_key, "sk-your-key");
  assert.equal(parsed.translation.model, "gpt-4o-mini");
  assert.equal(parsed.translation.temperature, 0.2);
  assert.equal(parsed.translation.timeout, 120);
});

test("a quoted value keeps a hash and a table keeps its headers", () => {
  const parsed = parseToml(
    [
      "[translation]",
      "base_url = 'https://example.test/v1'",
      'model = "m#1" # a comment',
      "temperature = 0.3",
      "",
      "[translation.headers]",
      'X-Key = "a#b"',
    ].join("\n"),
  );
  assert.equal(parsed.translation.base_url, "https://example.test/v1");
  assert.equal(parsed.translation.model, "m#1");
  assert.equal(parsed.translation.temperature, 0.3);
  assert.equal(parsed.translation.headers["X-Key"], "a#b");
});

test("a bad line names its number", () => {
  assert.throws(() => parseToml('[translation]\nbase_url "x"\n'), /line 2/u);
  assert.throws(() => parseToml('[translation]\nbase_url = "x\n'), /line 2/u);
  assert.equal(
    parseToml('\uFEFF[translation]\nmodel = "m"').translation.model,
    "m",
  );
});

test("the endpoint is the completion path", () => {
  assert.equal(
    endpoint("https://api.test/v1/"),
    "https://api.test/v1/chat/completions",
  );
  assert.equal(
    endpoint("https://api.test/v1/chat/completions"),
    "https://api.test/v1/chat/completions",
  );
});

test("the data home names the first candidate", () => {
  const previous = process.env.ROLA_DATA_HOME;
  const home = path.join(os.tmpdir(), "rola-data-home-test");
  process.env.ROLA_DATA_HOME = home;
  try {
    assert.equal(
      configCandidates()[0],
      path.join(path.resolve(home), "rola", "dev", "llm-tools.toml"),
    );
  } finally {
    if (previous === undefined) {
      delete process.env.ROLA_DATA_HOME;
    } else {
      process.env.ROLA_DATA_HOME = previous;
    }
  }
});

test("a configuration is read and checked", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "llm-tools-"));
  const file = path.join(directory, "llm-tools.toml");
  try {
    fs.writeFileSync(
      file,
      [
        "[translation]",
        'base_url = "http://127.0.0.1:9/v1"',
        'api_key = "key"',
        'model = "model"',
        "temperature = 0",
        "timeout = 30",
        "",
        "[translation.headers]",
        'X-Test = "yes"',
      ].join("\n"),
    );
    const config = readTranslationConfig([file]);
    assert.equal(config.baseUrl, "http://127.0.0.1:9/v1");
    assert.equal(config.model, "model");
    assert.equal(config.apiKey, "key");
    assert.equal(config.temperature, 0);
    assert.equal(config.timeout, 30);
    assert.equal(config.headers["X-Test"], "yes");

    assert.throws(
      () =>
        readTranslationConfig([
          path.join(directory, "missing", "llm-tools.toml"),
        ]),
      (error) => error.status === 503 && /llm-tools\.toml/u.test(error.message),
    );

    fs.writeFileSync(file, '[translation]\nbase_url = "http://x/v1"\n');
    assert.throws(
      () => readTranslationConfig([file]),
      (error) =>
        error.status === 503 && /model is missing/u.test(error.message),
    );
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
