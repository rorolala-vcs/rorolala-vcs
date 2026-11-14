import assert from "node:assert/strict";
import test from "node:test";

import { renderLines, renderPlain } from "../lib/render.mjs";
import { cellsOf, displayWidth } from "../lib/width.mjs";

const text = (source) => renderPlain(source, { theme: "text", color: false });

test("a message is drawn with its markers gone", () => {
  assert.equal(text("Hello, **world**!"), "Hello, world!");
  assert.equal(text("Hello, *world*!"), "Hello, world!");
  assert.equal(text("Hello, ***world***!"), "Hello, world!");
  assert.equal(text("Hello, __world__!"), "Hello, world!");
  assert.equal(text("Hello, ~~world~~!"), "Hello, world!");
  assert.equal(text("2 * 3 and a < b"), "2 * 3 and a < b");
});

test("a code span, a flag and a link keep the text a terminal shows", () => {
  assert.equal(text("`a\\*b` and <FILE>"), "`a\\*b` and <FILE>");
  assert.equal(text("[[?https://example.com]]the text[[/]]"), "the text");
  assert.equal(text("\\*not italic\\*"), "*not italic*");
  assert.equal(text("\\u2764 and \\u{2764}"), "\u2764 and \u2764");
  assert.equal(text("\\ud83d\\ude00"), "\u{1f600}");
});

test("a heading is a title with a line either side", () => {
  assert.equal(text("text\n# Title\ntext"), "text\n\nTitle\n\ntext");
  assert.equal(text("# Title\n\nText"), "Title\n\nText");
  assert.equal(text("#### Not bold, still spaced"), "Not bold, still spaced");
  assert.equal(text("#no space"), "#no space");
  assert.equal(text("####### too many"), "####### too many");
});

test("a quotation and an alert use the gutter the renderer draws", () => {
  assert.equal(text("> quoted"), "| quoted");
  assert.equal(text("> first\n>\n> third"), "| first\n|\n| third");
  assert.equal(
    text("> [!Warning]\n>\n> something went wrong"),
    "| WARNING:\n|\n| something went wrong",
  );
  assert.equal(text("> [!nonsense]\n> text"), "| [!nonsense]\n| text");
  assert.equal(
    renderPlain("> [!Warning]\n>\n> something went wrong", {
      theme: "pretty",
      color: false,
    }),
    "| \u{f071} Warning\n|\n| something went wrong",
  );
});

test("a rule is as wide as the message, or as the terminal where it is known", () => {
  assert.equal(text("Hello\n----\nWorld"), "Hello\n-----\nWorld");
  assert.equal(text("Hello\n---\nWorld"), "Hello\n-----\nWorld");
  assert.equal(
    renderPlain("Hello\n----\nWorld", { theme: "pretty", color: false }),
    "Hello\n\u2500\u2500\u2500\u2500\u2500\nWorld",
  );
  assert.equal(
    renderPlain("Hello\n----\nWorld", {
      theme: "pretty",
      color: false,
      terminal: 12,
    }),
    "Hello\n\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\nWorld",
  );
});

test("a pipe table is drawn with its columns aligned", () => {
  const drawn = text("| Name | Value |\n| :--- | ---: |\n| a | 1 |");
  const lines = drawn.split("\n");
  assert.equal(lines.length, 5);
  assert.ok(lines[1].includes("| Name | Value |"), drawn);
  assert.ok(lines[3].includes("| a    |     1 |"), drawn);

  const box = renderPlain("| Name | Value |\n| --- | --- |\n| a | 1 |", {
    theme: "pretty",
    color: false,
  });
  assert.ok(box.startsWith("\u250c"), box);
  assert.ok(box.includes("\u251c"), box);
  assert.ok(box.endsWith("\u2518"), box);
});

test("a fenced block is drawn without its fence", () => {
  assert.equal(text("```\none\ntwo\n```"), "one\ntwo");
  assert.equal(text("```\none\ntwo"), "one\ntwo");
});

test("a flag closes at its own marker", () => {
  const drawn = renderLines("`code` and <FLAG> and a < b", {
    theme: "simple",
    color: true,
  });
  const plain = drawn.lines[0].find((run) => run.text.includes("a < b"));
  assert.ok(plain);
  assert.equal(plain.style.italic, false);
  assert.equal(plain.style.fg, null);
});

test("a table with two body rows separates them", () => {
  const drawn = text("| Name | Value |\n| --- | --- |\n| a | 1 |\n| b | 2 |");
  const lines = drawn.split("\n");
  assert.equal(lines.length, 7);
  assert.equal(lines[2], "+------+-------+");
  assert.equal(lines[4], "+------+-------+");
  assert.equal(lines[6], "+------+-------+");
});

test("a link inside a table keeps its text and drops the address", () => {
  const drawn = renderLines(
    "| A | B |\n| --- | --- |\n| [[?https://example.com]]x[[/]] | y |",
    { theme: "simple", color: true },
  );
  assert.equal(
    drawn.lines.flat().filter((run) => run.style.link !== null).length,
    0,
  );
  assert.ok(drawn.lines[3].some((run) => run.text.includes("x")));
});

test("a fenced block expands its tabs as bat does", () => {
  assert.equal(text("```\n\ta\n```"), "    a");
  assert.equal(text("```\n  \ta\n```"), "    a");
  assert.equal(text("```\nab\tc\n```"), "ab  c");
});

test("a colour a simple terminal cannot ask for moves to the nearest of the sixteen", () => {
  const red = renderLines("[[#ff0000]]x[[/]]", {
    theme: "simple",
    color: true,
  });
  assert.deepEqual(red.lines[0][0].style.fg, { kind: "named", index: 9 });

  const orange = renderLines("[[#ff8800]]x[[/]]", {
    theme: "simple",
    color: true,
  });
  assert.deepEqual(orange.lines[0][0].style.fg, { kind: "named", index: 3 });
});

test("a named colour is an exact one where the terminal carries true colour", () => {
  const drawn = renderLines("[[red]]x[[/]]", {
    theme: "pretty",
    color: true,
  });
  assert.deepEqual(drawn.lines[0][0].style.fg, {
    kind: "exact",
    red: 0xcd,
    green: 0,
    blue: 0,
  });
});

test("a trailing space inside a style is kept, as the painted line keeps it", () => {
  const source = "2 * 3 and an unmatched ** and a lone *";
  assert.equal(
    renderPlain(source, { theme: "simple", color: true }),
    "2  3 and an unmatched ** and a lone ",
  );
  assert.equal(text(source), "2  3 and an unmatched ** and a lone");
});

test("a braced character escape leaves the character after it alone", () => {
  assert.equal(text("\\u{2764} x"), "\u2764 x");
});

test("a style is kept for the preview and dropped by the flat theme", () => {
  const simple = renderLines("[[red]]red[[/]]", {
    theme: "simple",
    color: true,
  });
  assert.deepEqual(simple.lines[0][0].style.fg, { kind: "named", index: 1 });

  const exact = renderLines("[[#ff0000]]red[[/]]", {
    theme: "pretty",
    color: true,
  });
  assert.deepEqual(exact.lines[0][0].style.fg, {
    kind: "exact",
    red: 255,
    green: 0,
    blue: 0,
  });

  const flat = renderLines("[[red]]red[[/]]", { theme: "text", color: true });
  assert.equal(flat.lines[0][0].style.fg, null);
  assert.equal(flat.lines[0][0].style.bold, false);
});

test("the desktop theme is the source as it was written", () => {
  assert.equal(renderPlain("  **raw**  \n", { theme: "desktop" }), "**raw**");
});

test("a wide character is two columns and a combining mark is none", () => {
  assert.equal(displayWidth("ab"), 2);
  assert.equal(displayWidth("中文"), 4);
  assert.equal(displayWidth("e\u0301"), 1);
  assert.equal(displayWidth("\u{1f600}"), 2);
});

test("the preview draws one box per character", () => {
  const usage = cellsOf("Usage");
  assert.deepEqual(
    usage.map((cell) => cell.cells),
    [1, 1, 1, 1, 1],
  );
  assert.deepEqual(
    usage.map((cell) => cell.text),
    ["U", "s", "a", "g", "e"],
  );

  const chinese = cellsOf("中文");
  assert.deepEqual(
    chinese.map((cell) => cell.cells),
    [2, 2],
  );

  for (const source of [
    "Usage: rola account current",
    "中文 and ASCII",
    "e\u0301",
    "a\tb",
    "\u{1f600}x",
  ]) {
    const cells = cellsOf(source);
    assert.equal(
      cells.reduce((sum, cell) => sum + cell.cells, 0),
      displayWidth(source),
      source,
    );
    assert.equal(cells.map((cell) => cell.text).join(""), source);
  }
});
