import assert from "node:assert/strict";
import test from "node:test";

import { analyzeKey, summarize } from "../lib/analyze.mjs";

function key(id, kind, en, zh) {
  const langs = {};
  if (en !== undefined) {
    langs.en = { value: en, style: "block", line: 1, editable: true };
  }
  if (zh !== undefined) {
    langs["zh-CN"] = { value: zh, style: "block", line: 10, editable: true };
  }
  return {
    id,
    kind,
    file: "sample.yml",
    path: id.split("."),
    langs,
    callsites: [],
  };
}

test("a placeholder in one language is a finding on the other", () => {
  const findings = analyzeKey(key("x", "message", "Hello %{name}", "你好"));
  const found = findings.find((finding) => finding.kind === "placeholder");
  assert.ok(found);
  assert.equal(found.lang, "en");
  assert.equal(found.message, "zh-CN lacks %{name}");
  assert.equal(found.offset, 6);
});

test("a missing language is a finding", () => {
  const findings = analyzeKey(key("x", "message", "Hello", undefined));
  assert.ok(findings.some((finding) => finding.kind === "missing" && finding.lang === "zh-CN"));
});

test("the banned dashes are reported, but not inside a code span", () => {
  const findings = analyzeKey(key("x", "message", "a \u2014 b", "a \u2014 b"));
  assert.ok(
    findings.some(
      (finding) => finding.kind === "copy" && finding.message.includes("U+2014"),
    ),
  );

  const masked = analyzeKey(key("y", "message", "`\u2014`", "`\u2014`"));
  assert.equal(masked.some((finding) => finding.kind === "copy"), false);
});

test("a help block whose right column differs is a finding", () => {
  const value = "Flags:\n    --a      one\n    --bbb   two";
  const findings = analyzeKey(key("x.help", "help", value, value));
  const found = findings.find((finding) => finding.kind === "alignment");
  assert.ok(found);
  assert.match(found.message, /right column/u);
});

test("a line past eighty columns is reported, not rejected", () => {
  const value = "a".repeat(90);
  const findings = analyzeKey(key("x.help", "help", value, "short"));
  const found = findings.find((finding) => finding.kind === "overlong");
  assert.ok(found);
  assert.equal(found.severity, "info");
});

test("a summary counts severities and checks", () => {
  const findings = [
    { severity: "warn", kind: "alignment" },
    { severity: "info", kind: "copy" },
    { severity: "info", kind: "copy" },
  ];
  assert.deepEqual(summarize(findings), {
    total: 3,
    warn: 1,
    info: 2,
    kinds: { alignment: 1, copy: 2 },
  });
});
