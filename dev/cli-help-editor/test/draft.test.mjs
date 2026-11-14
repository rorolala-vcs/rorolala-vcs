import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { analyzeKey } from "../lib/analyze.mjs";
import { DRAFT_FILE, DRAFT_ID, readState, writeEdits } from "../lib/scan.mjs";

function project() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cli-help-draft-"));
  const i18n = path.join(root, "app", "cli", "rola", "i18n");
  fs.mkdirSync(i18n, { recursive: true });
  fs.writeFileSync(path.join(root, "Cargo.toml"), "");
  fs.writeFileSync(
    path.join(i18n, "cmd_x.yml"),
    [
      "_version: 2",
      "",
      "x:",
      "  help:",
      "    en: |",
      "        Show x",
      "    zh-CN: |",
      "        显示 x",
      "",
    ].join("\n"),
  );
  return root;
}

function draftEdit(state, value, expectedHash = state.fileHashes[DRAFT_FILE]) {
  return [
    {
      file: DRAFT_FILE,
      id: "draft",
      lang: "draft",
      value,
      expectedHash,
    },
  ];
}

test("the draft is listed first and starts empty", () => {
  const root = project();
  try {
    const state = readState(root);
    assert.equal(state.keys[0].id, DRAFT_ID);
    assert.equal(state.keys[0].kind, "draft");
    assert.equal(state.keys[0].file, DRAFT_FILE);
    assert.equal(state.keys[0].langs.draft.value, "");
    assert.equal(state.keys[0].langs.draft.editable, true);
    assert.equal(state.fileHashes[DRAFT_FILE], "");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("a save writes the version and the draft, and reads them back", () => {
  const root = project();
  try {
    const before = readState(root);
    const after = writeEdits(root, draftEdit(before, "Hello\nworld"));
    assert.equal(
      fs.readFileSync(path.join(root, DRAFT_FILE), "utf8"),
      ["version: 1", "", "draft: |", "    Hello", "    world", ""].join("\n"),
    );
    assert.equal(after.keys[0].langs.draft.value, "Hello\nworld");
    assert.notEqual(after.fileHashes[DRAFT_FILE], "");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("a save keeps the version the file states", () => {
  const root = project();
  try {
    fs.mkdirSync(path.dirname(path.join(root, DRAFT_FILE)), {
      recursive: true,
    });
    fs.writeFileSync(
      path.join(root, DRAFT_FILE),
      "version: 7\n\ndraft: |\n    old\n",
    );
    const before = readState(root);
    assert.equal(before.keys[0].langs.draft.value, "old");

    writeEdits(root, draftEdit(before, "new"));
    const text = fs.readFileSync(path.join(root, DRAFT_FILE), "utf8");
    assert.match(text, /^version: 7\n/u);
    assert.match(text, /draft: \|\n {4}new\n$/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("the draft and an i18n key save in one batch", () => {
  const root = project();
  try {
    const before = readState(root);
    const key = before.keys.find((candidate) => candidate.key === "x.help");
    const after = writeEdits(root, [
      ...draftEdit(before, "drafted"),
      {
        file: key.file,
        id: "x.help",
        lang: "en",
        value: "Show y",
        expectedHash: before.fileHashes[key.file],
      },
    ]);
    assert.equal(after.keys[0].langs.draft.value, "drafted");
    assert.equal(
      after.keys.find((candidate) => candidate.key === "x.help").langs.en.value,
      "Show y",
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("a draft file changed under the editor is refused", () => {
  const root = project();
  try {
    const before = readState(root);
    writeEdits(root, draftEdit(before, "one"));
    const stale = readState(root);
    fs.appendFileSync(path.join(root, DRAFT_FILE), "    and another\n");
    assert.throws(
      () => writeEdits(root, draftEdit(stale, "two")),
      (error) => error.status === 409,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("an empty draft is not a message missing its words", () => {
  const root = project();
  try {
    const state = readState(root);
    const findings = analyzeKey(state.keys[0], { maxWidth: 80 });
    assert.equal(
      findings.some((found) => found.kind === "empty"),
      false,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("the draft is checked as a help page, in the language it holds", () => {
  const root = project();
  try {
    const before = readState(root);
    const state = writeEdits(
      root,
      draftEdit(before, `# Title\n**Usage**: rola x\n${"word ".repeat(20)}`),
    );
    const findings = analyzeKey(state.keys[0], { maxWidth: 80 });
    assert.equal(
      findings.some((found) => found.kind === "missing"),
      false,
    );
    assert.equal(
      findings.some((found) => found.kind === "overlong"),
      true,
    );
    assert.equal(
      findings.every((found) => found.lang === "draft"),
      true,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
