import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { findRoot } from "../lib/scan.mjs";
import {
  applyEdit,
  encodeQuoted,
  joinSource,
  parseFile,
  safePlain,
  splitSource,
} from "../lib/yaml.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));

function roundTrip(source, file = "sample.yml") {
  const parsed = parseFile(source, file);
  const nodes = [...parsed.nodes].sort(
    (left, right) => right.keyLine - left.keyLine,
  );
  for (const node of nodes) {
    if (!node.editable) {
      continue;
    }
    assert.equal(applyEdit(parsed.lines, node, node.value), true);
  }
  return joinSource(parsed);
}

const SAMPLE = `_version: 2

# A comment that has to stay.
checkin:
  help:
    en: |
        Line one
        Line two

        After a gap
    zh-CN: |
        第一行
  err_x:
    en: "a \\"quoted\\" value"
    zh-CN: 纯文本
  complete:
    by_name:
      en: |
          A name of its own
      zh-CN: 自己的名字
`;

test("the three scalar styles are read and written back as they are", () => {
  const parsed = parseFile(SAMPLE, "sample.yml");
  const help = parsed.nodes.find(
    (node) => node.id === "checkin.help" && node.lang === "en",
  );
  assert.equal(help.value, "Line one\nLine two\n\nAfter a gap");
  assert.equal(help.style, "block");
  assert.equal(help.contentIndent, 8);

  const quoted = parsed.nodes.find(
    (node) => node.id === "checkin.err_x" && node.lang === "en",
  );
  assert.equal(quoted.value, 'a "quoted" value');
  assert.equal(quoted.style, "quoted");

  const plain = parsed.nodes.find(
    (node) => node.id === "checkin.err_x" && node.lang === "zh-CN",
  );
  assert.equal(plain.value, "纯文本");
  assert.equal(plain.style, "plain");

  const nested = parsed.nodes.find(
    (node) => node.id === "checkin.complete.by_name" && node.lang === "en",
  );
  assert.equal(nested.value, "A name of its own");
  assert.equal(nested.contentIndent, 10);

  assert.equal(roundTrip(SAMPLE), SAMPLE);
});

test("a changed block keeps its indentation and leaves the rest of the file alone", () => {
  const parsed = parseFile(SAMPLE, "sample.yml");
  const node = parsed.nodes.find(
    (candidate) => candidate.id === "checkin.help" && candidate.lang === "en",
  );
  applyEdit(parsed.lines, node, "New one\n\nNew three");
  const written = joinSource(parsed);

  assert.ok(
    written.includes(
      "    en: |\n        New one\n\n        New three\n    zh-CN: |\n",
    ),
  );
  assert.ok(written.includes("# A comment that has to stay."));
  assert.ok(written.endsWith("\n"));
});

test("a quote and a colon in a plain value become a quoted scalar", () => {
  assert.equal(safePlain("plain words"), true);
  assert.equal(safePlain("- a list"), false);
  assert.equal(safePlain("a: b"), false);
  assert.equal(safePlain("one\ntwo"), false);

  const source = `_version: 2\nx:\n  en: plain words\n  zh-CN: plain words\n`;
  const parsed = parseFile(source, "sample.yml");
  const node = parsed.nodes.find((candidate) => candidate.lang === "en");
  applyEdit(parsed.lines, node, 'a: "b"');
  const written = joinSource(parsed);
  assert.ok(written.includes('en: "a: \\"b\\""'));
});

test("a quoted scalar keeps its line and takes escaped newlines", () => {
  const source = `_version: 2\nx:\n  en: "one"\n  zh-CN: "一"\n`;
  const parsed = parseFile(source, "sample.yml");
  const node = parsed.nodes.find((candidate) => candidate.lang === "en");
  applyEdit(parsed.lines, node, "one\ntwo");
  const written = joinSource(parsed);

  assert.ok(written.includes('en: "one\\ntwo"'));
  assert.equal(encodeQuoted('a "b"\\c'), 'a \\"b\\"\\\\c');
});

test("a carriage return file stays a carriage return file", () => {
  const source = "_version: 2\r\nx:\r\n  en: |\r\n      one\r\n  zh-CN: 一\r\n";
  const parsed = parseFile(source, "sample.yml");
  assert.equal(parsed.eol, "\r\n");
  const node = parsed.nodes.find((candidate) => candidate.lang === "en");
  applyEdit(parsed.lines, node, "one");
  assert.equal(joinSource(parsed), source);
  assert.equal(splitSource(source).lines.length, 6);
});

test("an empty scalar is kept as it is and left alone", () => {
  const source = "_version: 2\nx:\n  en:\n  zh-CN: 一\n";
  const parsed = parseFile(source, "sample.yml");
  const en = parsed.nodes.find((node) => node.lang === "en");
  assert.equal(en.style, "unsupported");
  assert.equal(en.editable, false);
  assert.ok(
    parsed.problems.some((problem) => problem.message.includes("empty scalar")),
  );
  assert.equal(roundTrip(source), source);
});

test("every file in the tree is written back unchanged", () => {
  const root = findRoot(HERE);
  const trees = ["rola", "rola-daemon", "git-rola", "rola-edit"]
    .map((name) => path.join(root, "app", "cli", name, "i18n"))
    .filter((directory) => fs.existsSync(directory));
  const files = [];

  const walk = (directory) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      if (entry.name.startsWith("__")) {
        continue;
      }
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        walk(absolute);
      } else if (entry.isFile() && entry.name.endsWith(".yml")) {
        files.push(absolute);
      }
    }
  };
  trees.forEach(walk);

  assert.ok(files.length > 0);
  let nodes = 0;
  for (const file of files) {
    const source = fs.readFileSync(file, "utf8");
    const written = roundTrip(source, path.relative(root, file));
    assert.equal(written, source, path.relative(root, file));
    nodes += parseFile(source, file).nodes.length;
  }
  assert.ok(nodes > 1000, `read ${nodes} values`);
});
