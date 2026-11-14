import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { EditorError, readState, writeEdits } from "../lib/scan.mjs";

const FILE = "app/cli/rola/i18n/cmd_test.yml";

function project() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cli-help-editor-"));
  fs.mkdirSync(path.join(root, "app", "cli", "rola", "i18n"), {
    recursive: true,
  });
  fs.mkdirSync(path.join(root, "app", "cli", "rola", "src"), {
    recursive: true,
  });
  fs.writeFileSync(
    path.join(root, "app", "cli", "rola", "src", "cmd_test.rs"),
    'pub fn help_test() { let _ = t!("test.help"); }\n',
  );
  fs.writeFileSync(
    path.join(root, FILE),
    `_version: 2

test:
  help:
    en: |
        Old help

        **Usage**: rola test
    zh-CN: |
        旧帮助
  err_x:
    en: "a value"
    zh-CN: 一个值
`,
  );
  return root;
}

test("a save reads one file, writes one key and leaves the rest", () => {
  const root = project();
  try {
    const state = readState(root);
    assert.equal(state.keys.length, 3);
    assert.equal(state.keys[0].kind, "draft");
    const help = state.keys.find((key) => key.key === "test.help");
    assert.equal(help.kind, "help");
    assert.equal(help.callsites.length, 1);
    assert.equal(help.langs.en.line, 5);

    const written = writeEdits(root, [
      {
        file: FILE,
        id: "test.help",
        lang: "en",
        value: "New help\n\n**Usage**: rola test name",
        expectedHash: state.fileHashes[FILE],
      },
    ]);

    const found = written.keys.find((key) => key.key === "test.help");
    assert.equal(found.langs.en.value, "New help\n\n**Usage**: rola test name");
    const text = fs.readFileSync(path.join(root, FILE), "utf8");
    assert.ok(
      text.includes(
        "        New help\n\n        **Usage**: rola test name\n    zh-CN: |\n",
      ),
    );
    assert.ok(
      text.includes('  err_x:\n    en: "a value"\n    zh-CN: 一个值\n'),
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("a file that changed under the editor is refused", () => {
  const root = project();
  try {
    const state = readState(root);
    fs.writeFileSync(
      path.join(root, FILE),
      fs
        .readFileSync(path.join(root, FILE), "utf8")
        .replace("Old help", "Changed outside"),
    );

    assert.throws(
      () =>
        writeEdits(root, [
          {
            file: FILE,
            id: "test.help",
            lang: "en",
            value: "New help",
            expectedHash: state.fileHashes[FILE],
          },
        ]),
      (error) => error instanceof EditorError && error.status === 409,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("a path outside the project is refused", () => {
  const root = project();
  try {
    assert.throws(
      () =>
        writeEdits(root, [
          {
            file: "../outside.yml",
            id: "test.help",
            lang: "en",
            value: "x",
            expectedHash: "x",
          },
        ]),
      (error) => error instanceof EditorError,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
