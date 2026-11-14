// A port of `utils/cli-theme`'s `TextRendering::render`, for the browser and for
// the checks. The functions here hand back styled runs rather than escapes, so the
// editor draws them as spans and the analysis measures them. The text, the block
// shapes and the colours are the ones the Rust renderer draws.
//
// The port covers the message path only: `render`, not `render_document`. A help
// page and a status line are drawn by `render`, and the document layout belongs to
// `rola docs`, which this editor does not open.

import { charWidth, displayWidth } from "./width.mjs";

/** The colours a terminal falls back to for the sixteen. */
export const STANDARD = [
  [0x00, 0x00, 0x00],
  [0xcd, 0x00, 0x00],
  [0x00, 0xcd, 0x00],
  [0xcd, 0xcd, 0x00],
  [0x00, 0x00, 0xee],
  [0xcd, 0x00, 0xcd],
  [0x00, 0xcd, 0xcd],
  [0xe5, 0xe5, 0xe5],
  [0x7f, 0x7f, 0x7f],
  [0xff, 0x00, 0x00],
  [0x00, 0xff, 0x00],
  [0xff, 0xff, 0x00],
  [0x5c, 0x5c, 0xff],
  [0xff, 0x00, 0xff],
  [0x00, 0xff, 0xff],
  [0xff, 0xff, 0xff],
];

const BASE_COLORS = {
  black: 0,
  red: 1,
  green: 2,
  yellow: 3,
  blue: 4,
  magenta: 5,
  cyan: 6,
  white: 7,
};

const BRIGHT_CYAN = { kind: "named", index: 14 };
const GREY = { kind: "named", index: 8 };

const ALERTS = {
  important: {
    name: "IMPORTANT",
    title: "Important",
    glyph: "\u{f03eb}",
    color: 5,
  },
  tip: { name: "TIP", title: "Tip", glyph: "\u{f0335}", color: 10 },
  note: { name: "NOTE", title: "Note", glyph: "\u{f00ba}", color: 14 },
  warning: { name: "WARNING", title: "Warning", glyph: "\u{f071}", color: 11 },
  caution: { name: "CAUTION", title: "Caution", glyph: "\u{ea6c}", color: 9 },
};

const DELIMITERS = ["***", "**", "__", "~~", "*"];
const ESCAPABLE = new Set(["*", "_", "~", "\\"]);
const CODE_TAB_WIDTH = 4;

const CODE = { kind: "code", open: "`", close: "`" };
const FLAG = { kind: "flag", open: "<", close: ">" };

function plainStyle() {
  return {
    fg: null,
    bg: null,
    bold: false,
    italic: false,
    underline: false,
    strike: false,
    reverse: false,
    link: null,
  };
}

function copyStyle(style) {
  return { ...style };
}

function namedColor(name) {
  const base = BASE_COLORS[name.toLowerCase()];
  if (base === undefined) {
    return null;
  }
  const bright = name === name.toUpperCase() && name !== name.toLowerCase();
  return { kind: "named", index: bright ? base + 8 : base };
}

function exactColor(text) {
  if (!text.startsWith("#") || text.length !== 7) {
    return null;
  }
  if (!/^[0-9a-fA-F]{6}$/.test(text.slice(1))) {
    return null;
  }
  return {
    kind: "exact",
    red: Number.parseInt(text.slice(1, 3), 16),
    green: Number.parseInt(text.slice(3, 5), 16),
    blue: Number.parseInt(text.slice(5, 7), 16),
  };
}

function color(text) {
  return exactColor(text) ?? namedColor(text);
}

/** The nearest of the sixteen, for a theme that cannot ask for an exact colour. */
function nearestIndex(rgb) {
  let best = 0;
  let distance = Number.POSITIVE_INFINITY;
  STANDARD.forEach((candidate, index) => {
    const red = candidate[0] - rgb[0];
    const green = candidate[1] - rgb[1];
    const blue = candidate[2] - rgb[2];
    const found = red * red + green * green + blue * blue;
    if (found < distance) {
      distance = found;
      best = index;
    }
  });
  return best;
}

/** The RGB channels a colour is drawn in for `theme`. */
export function rgbOf(colorValue, theme) {
  if (!colorValue) {
    return null;
  }
  if (colorValue.kind === "named") {
    return STANDARD[colorValue.index] ?? STANDARD[0];
  }
  const rgb = [colorValue.red, colorValue.green, colorValue.blue];
  return theme === "pretty" ? rgb : STANDARD[nearestIndex(rgb)];
}

function namedOf(colorValue) {
  return {
    kind: "named",
    index: nearestIndex([colorValue.red, colorValue.green, colorValue.blue]),
  };
}

function exactOf(colorValue) {
  const rgb = STANDARD[colorValue.index] ?? STANDARD[0];
  return { kind: "exact", red: rgb[0], green: rgb[1], blue: rgb[2] };
}

function normalizeStyle(style, options) {
  if (options.theme === "text" || options.color === false) {
    return plainStyle();
  }
  const normalized = copyStyle(style);
  if (options.theme === "simple") {
    if (normalized.fg?.kind === "exact") {
      normalized.fg = namedOf(normalized.fg);
    }
    if (normalized.bg?.kind === "exact") {
      normalized.bg = namedOf(normalized.bg);
    }
  } else if (options.theme === "pretty") {
    if (normalized.fg?.kind === "named") {
      normalized.fg = exactOf(normalized.fg);
    }
    if (normalized.bg?.kind === "named") {
      normalized.bg = exactOf(normalized.bg);
    }
  }
  if (options.hyperlinks === false) {
    normalized.link = null;
  }
  return normalized;
}

function paint(text, style, options) {
  return { text, style: normalizeStyle(style, options) };
}

function styleKey(style) {
  const colorKey = (value) =>
    value === null
      ? ""
      : value.kind === "named"
        ? `n${value.index}`
        : `e${value.red},${value.green},${value.blue}`;
  return [
    colorKey(style.fg),
    colorKey(style.bg),
    style.bold ? "b" : "",
    style.italic ? "i" : "",
    style.underline ? "u" : "",
    style.strike ? "s" : "",
    style.reverse ? "r" : "",
    style.link ?? "",
  ].join("|");
}

function pushRun(runs, text, style) {
  if (text === "") {
    return;
  }
  const last = runs[runs.length - 1];
  if (last && styleKey(last.style) === styleKey(style)) {
    last.text += text;
  } else {
    runs.push({ text, style });
  }
}

function wrapperStyle(state, wrapper) {
  const styled = copyStyle(state);
  styled.fg = BRIGHT_CYAN;
  if (wrapper.kind === "code") {
    styled.bold = true;
  } else {
    styled.italic = true;
  }
  return styled;
}

function attributesOf(delimiter) {
  switch (delimiter) {
    case "***":
      return { bold: true, italic: true, underline: false, strike: false };
    case "**":
      return { bold: true, italic: false, underline: false, strike: false };
    case "__":
      return { bold: false, italic: false, underline: true, strike: false };
    case "~~":
      return { bold: false, italic: false, underline: false, strike: true };
    default:
      return { bold: false, italic: true, underline: false, strike: false };
  }
}

function attributesActive(attributes, state) {
  return (
    (!attributes.bold || state.bold) &&
    (!attributes.italic || state.italic) &&
    (!attributes.underline || state.underline) &&
    (!attributes.strike || state.strike)
  );
}

function attributesOn(attributes, state) {
  return {
    ...state,
    bold: state.bold || attributes.bold,
    italic: state.italic || attributes.italic,
    underline: state.underline || attributes.underline,
    strike: state.strike || attributes.strike,
  };
}

function attributesOff(attributes, state) {
  return {
    ...state,
    bold: state.bold && !attributes.bold,
    italic: state.italic && !attributes.italic,
    underline: state.underline && !attributes.underline,
    strike: state.strike && !attributes.strike,
  };
}

function codeUnit(rest) {
  if (!rest.startsWith("\\u")) {
    return null;
  }
  const after = rest.slice(2);
  if (after.startsWith("{")) {
    const braced = after.slice(1);
    const end = braced.indexOf("}");
    if (end < 0) {
      return null;
    }
    const digits = braced.slice(0, end);
    if (!/^[0-9a-fA-F]+$/.test(digits)) {
      return null;
    }
    return { unit: Number.parseInt(digits, 16), consumed: end + 4 };
  }
  if (after.length < 4 || !/^[0-9a-fA-F]{4}$/.test(after.slice(0, 4))) {
    return null;
  }
  return { unit: Number.parseInt(after.slice(0, 4), 16), consumed: 6 };
}

function unicodeEscape(rest) {
  const high = codeUnit(rest);
  if (high === null) {
    return null;
  }
  if (high.unit >= 0xd800 && high.unit <= 0xdbff) {
    const low = codeUnit(rest.slice(high.consumed));
    if (low === null || low.unit < 0xdc00 || low.unit > 0xdfff) {
      return null;
    }
    const joined = 0x10000 + ((high.unit - 0xd800) << 10) + (low.unit - 0xdc00);
    return {
      character: String.fromCodePoint(joined),
      consumed: high.consumed + low.consumed,
    };
  }
  if (high.unit >= 0xd800 && high.unit <= 0xdfff) {
    return null;
  }
  if (high.unit > 0x10ffff) {
    return null;
  }
  return {
    character: String.fromCodePoint(high.unit),
    consumed: high.consumed,
  };
}

function inlineRender(text, base, options) {
  const runs = [];
  let position = 0;
  let literal = "";
  let state = copyStyle(base);
  const open = [];

  const flush = () => {
    if (literal !== "") {
      pushRun(runs, literal, normalizeStyle(state, options));
      literal = "";
    }
  };

  const inCode = () =>
    open.some((entry) => entry.by === "wrapper" && entry.wrapper === CODE);

  const advance = () => {
    const character = String.fromCodePoint(text.codePointAt(position));
    literal += character;
    position += character.length;
  };

  const close = (kind, wrapper) => {
    let at = -1;
    for (let index = open.length - 1; index >= 0; index -= 1) {
      const entry = open[index];
      if (
        entry.by === kind &&
        (wrapper === undefined || entry.wrapper === wrapper)
      ) {
        at = index;
        break;
      }
    }
    if (at < 0) {
      return;
    }
    const saved = open[at].state;
    open.length = at;
    state = saved;
  };

  const escape = (rest) => {
    const unicode = unicodeEscape(rest);
    if (unicode !== null) {
      literal += unicode.character;
      position += unicode.consumed;
      return;
    }
    const following = String.fromCodePoint(rest.codePointAt(1) ?? 0);
    if (ESCAPABLE.has(following)) {
      literal += following;
      position += 1 + following.length;
      return;
    }
    literal += "\\";
    position += 1;
  };

  const wrapperFor = (character) => {
    if (character === "`") {
      return CODE;
    }
    if (character === "<") {
      return FLAG;
    }
    return null;
  };

  const wrapperAt = (character, rest) => {
    const last = open[open.length - 1];
    if (last && last.by === "wrapper" && last.wrapper.close === character) {
      return last.wrapper;
    }
    const candidate = wrapperFor(character);
    if (candidate === null) {
      return null;
    }
    return rest.slice(character.length).includes(candidate.close)
      ? candidate
      : null;
  };

  const mark = (wrapper, character) => {
    const last = open[open.length - 1];
    if (last && last.by === "wrapper" && last.wrapper === wrapper) {
      literal += character;
      flush();
      close("wrapper", wrapper);
    } else {
      flush();
      open.push({ by: "wrapper", wrapper, state: copyStyle(state) });
      state = wrapperStyle(state, wrapper);
      literal += character;
    }
    position += character.length;
  };

  const directive = (rest) => {
    const end = rest.indexOf("]]");
    if (end < 0) {
      literal += "[[";
      position += 2;
      return;
    }
    const inner = rest.slice(2, end);
    let opened = null;
    if (inner === "/") {
      flush();
      close("directive");
      position += end + 2;
      return;
    }
    if (inner.startsWith("?")) {
      opened = copyStyle(state);
      opened.link = inner.slice(1);
    } else if (inner.startsWith("~")) {
      const found = color(inner.slice(1));
      if (found === null) {
        literal += "[[";
        position += 2;
        return;
      }
      opened = copyStyle(state);
      opened.bg = found;
    } else {
      const found = color(inner);
      if (found === null) {
        literal += "[[";
        position += 2;
        return;
      }
      opened = copyStyle(state);
      opened.fg = found;
    }
    flush();
    open.push({ by: "directive", state: copyStyle(state) });
    state = opened;
    position += end + 2;
  };

  const delimiter = (rest, found) => {
    const attributes = attributesOf(found);
    if (attributesActive(attributes, state)) {
      flush();
      state = attributesOff(attributes, state);
    } else if (rest.slice(found.length).includes(found)) {
      flush();
      state = attributesOn(attributes, state);
    } else {
      literal += found;
    }
    position += found.length;
  };

  while (position < text.length) {
    const rest = text.slice(position);
    const character = String.fromCodePoint(text.codePointAt(position));
    const literalNow = inCode();
    if (rest.startsWith("\\")) {
      if (literalNow) {
        advance();
      } else {
        escape(rest);
      }
    } else if (rest.startsWith("[[")) {
      directive(rest);
    } else {
      const wrapper = wrapperAt(character, rest);
      if (wrapper !== null) {
        mark(wrapper, character);
      } else if (literalNow) {
        advance();
      } else {
        const found = DELIMITERS.find((candidate) =>
          rest.startsWith(candidate),
        );
        if (found !== undefined) {
          delimiter(rest, found);
        } else {
          advance();
        }
      }
    }
  }
  flush();
  return runs;
}

function alertKind(line) {
  const trimmed = line.trim();
  if (!trimmed.startsWith("[!") || !trimmed.endsWith("]")) {
    return null;
  }
  return ALERTS[trimmed.slice(2, -1).trim().toLowerCase()] ?? null;
}

function alertTitle(kind, options) {
  return options.theme === "pretty"
    ? `${kind.glyph} ${kind.title}`
    : `${kind.name}:`;
}

function quoted(line, style, options) {
  if (line === "") {
    return [paint("|", style, options)];
  }
  return [paint("| ", style, options), ...inlineRender(line, style, options)];
}

function drawnQuote(body, options) {
  const lines = [];
  const kind = body.length === 0 ? null : alertKind(body[0].text);
  if (kind !== null) {
    const style = plainStyle();
    style.fg = { kind: "named", index: kind.color };
    lines.push(
      quoted(alertTitle(kind, options), { ...style, bold: true }, options),
    );
    for (const line of body.slice(1)) {
      lines.push(quoted(line.text, style, options));
    }
  } else {
    const grey = plainStyle();
    grey.fg = GREY;
    for (const line of body) {
      lines.push(quoted(line.text, grey, options));
    }
  }
  return lines.flatMap((runs, index) =>
    index === 0 ? runs : [{ text: "\n", style: plainStyle() }, ...runs],
  );
}

function heading(line) {
  let level = 0;
  while (level < line.length && line[level] === "#") {
    level += 1;
  }
  if (level === 0 || level > 6) {
    return null;
  }
  const rest = line.slice(level);
  if (!rest.startsWith(" ")) {
    return null;
  }
  const text = rest.slice(1).trim();
  return text === "" ? null : { level, text };
}

function isRule(line) {
  const trimmed = line.trim();
  return (
    trimmed.length >= 3 &&
    Array.from(trimmed).every((character) => character === "-")
  );
}

function isRow(line) {
  return line.trimStart().startsWith("|");
}

function splitRow(line) {
  let trimmed = line.trim();
  if (trimmed.startsWith("|")) {
    trimmed = trimmed.slice(1);
  }
  if (trimmed.endsWith("|")) {
    trimmed = trimmed.slice(0, -1);
  }
  return trimmed.split("|").map((cell) => cell.trim());
}

function isSeparatorCell(cell) {
  let dashes = cell.trim();
  while (dashes.startsWith(":")) {
    dashes = dashes.slice(1);
  }
  while (dashes.endsWith(":")) {
    dashes = dashes.slice(0, -1);
  }
  return (
    dashes.length >= 3 &&
    Array.from(dashes).every((character) => character === "-")
  );
}

function isSeparator(line) {
  const cells = splitRow(line);
  return cells.length > 0 && cells.every(isSeparatorCell);
}

function alignmentOf(cell) {
  const trimmed = cell.trim();
  if (trimmed.startsWith(":") && trimmed.endsWith(":")) {
    return "center";
  }
  if (trimmed.endsWith(":")) {
    return "right";
  }
  return "left";
}

const PLAIN_FRAME = {
  column: "|",
  line: "-",
  above: ["+", "+", "+"],
  between: ["+", "+", "+"],
  below: ["+", "+", "+"],
};

const BOX_FRAME = {
  column: "\u2502",
  line: "\u2500",
  above: ["\u250c", "\u252c", "\u2510"],
  between: ["\u251c", "\u253c", "\u2524"],
  below: ["\u2514", "\u2534", "\u2518"],
};

function frameOf(theme) {
  return theme === "pretty" ? BOX_FRAME : PLAIN_FRAME;
}

function ruleLine(widths, frame, corners) {
  let text = corners[0];
  widths.forEach((width, index) => {
    if (index > 0) {
      text += corners[1];
    }
    text += frame.line.repeat(width + 2);
  });
  text += corners[2];
  return [{ text, style: plainStyle() }];
}

function rowLine(cells, widths, alignments, frame) {
  const runs = [{ text: frame.column, style: plainStyle() }];
  widths.forEach((width, column) => {
    const content = cells[column] ?? [];
    const used = displayWidth(content.map((run) => run.text).join(""));
    const padding = Math.max(0, width - used);
    const alignment = alignments[column] ?? "left";
    const left =
      alignment === "right"
        ? padding
        : alignment === "center"
          ? Math.floor(padding / 2)
          : 0;
    const right = padding - left;
    runs.push({ text: " ", style: plainStyle() });
    if (left > 0) {
      runs.push({ text: " ".repeat(left), style: plainStyle() });
    }
    runs.push(...content);
    if (right > 0) {
      runs.push({ text: " ".repeat(right), style: plainStyle() });
    }
    runs.push({ text: " ", style: plainStyle() });
    runs.push({ text: frame.column, style: plainStyle() });
  });
  return runs;
}

function renderTable(lines, index, options) {
  const alignments = splitRow(lines[index + 1]).map(alignmentOf);
  const rows = [splitRow(lines[index])];
  let next = index + 2;
  while (next < lines.length && isRow(lines[next])) {
    rows.push(splitRow(lines[next]));
    next += 1;
  }

  const cellOptions = { ...options, hyperlinks: false };
  const drawn = rows.map((row) =>
    alignments.map((_, column) => {
      const text = row[column] ?? "";
      return inlineRender(text, plainStyle(), cellOptions);
    }),
  );
  const widths = alignments.map((_, column) =>
    Math.max(
      0,
      ...drawn.map((row) =>
        displayWidth((row[column] ?? []).map((run) => run.text).join("")),
      ),
    ),
  );
  const frame = frameOf(options.theme);
  const output = [];

  output.push({
    runs: ruleLine(widths, frame, frame.above),
    source: index,
  });
  output.push({
    runs: rowLine(drawn[0] ?? [], widths, alignments, frame),
    source: index,
  });
  output.push({
    runs: ruleLine(widths, frame, frame.between),
    source: index + 1,
  });
  const body = drawn.slice(1);
  body.forEach((row, offset) => {
    output.push({
      runs: rowLine(row, widths, alignments, frame),
      source: index + 2 + offset,
    });
    if (offset < body.length - 1) {
      output.push({
        runs: ruleLine(widths, frame, frame.between),
        source: index + 3 + offset,
      });
    }
  });
  output.push({
    runs: ruleLine(widths, frame, frame.below),
    source: Math.max(index, next - 1),
  });

  return { output, next };
}

function fenceOpening(line) {
  const marker = line[0];
  if (marker !== "`" && marker !== "~") {
    return null;
  }
  let length = 0;
  while (length < line.length && line[length] === marker) {
    length += 1;
  }
  if (length < 3) {
    return null;
  }
  const info = line.slice(length).trim();
  return {
    marker,
    length,
    language: info.split(/\s+/)[0] || null,
  };
}

function fenceCloses(fence, line) {
  const trimmed = line.trim();
  return (
    Array.from(trimmed).length >= fence.length &&
    Array.from(trimmed).every((character) => character === fence.marker)
  );
}

function takeFence(lines, index, fence) {
  const code = [];
  let next = index;
  while (next < lines.length) {
    if (fenceCloses(fence, lines[next])) {
      next += 1;
      break;
    }
    code.push(lines[next]);
    next += 1;
  }
  return { code: code.join("\n"), next, start: index, count: code.length };
}

function rustLines(text) {
  if (text === "") {
    return [];
  }
  const lines = text.split("\n");
  if (lines[lines.length - 1] === "") {
    lines.pop();
  }
  return lines.map((line) => (line.endsWith("\r") ? line.slice(0, -1) : line));
}

function expandTabs(text, width) {
  let expanded = "";
  let column = 0;
  for (const character of text) {
    if (character === "\t") {
      const spaces = width - (column % width);
      expanded += " ".repeat(spaces);
      column += spaces;
    } else {
      expanded += character;
      column += charWidth(character);
    }
  }
  return expanded;
}

function isStyled(style) {
  return (
    style.fg !== null ||
    style.bg !== null ||
    style.bold ||
    style.italic ||
    style.underline ||
    style.strike ||
    style.reverse ||
    style.link !== null
  );
}

function trimRunsEnd(runs) {
  const trimmed = [...runs];
  while (trimmed.length > 0) {
    const last = trimmed[trimmed.length - 1];
    if (isStyled(last.style)) {
      break;
    }
    const found = last.text.replace(/\s+$/u, "");
    if (found === last.text) {
      break;
    }
    if (found === "") {
      trimmed.pop();
    } else {
      trimmed[trimmed.length - 1] = { text: found, style: last.style };
      break;
    }
  }
  return trimmed;
}

function plainOf(runs) {
  return runs.map((run) => run.text).join("");
}

/**
 * Renders `source` the way `TextRendering::render` does.
 *
 * Returns the styled lines and, for each one, the source line it came from. A
 * table and an alert draw more than one line from one source line, so the map is
 * what lets a check point at the YAML line a reader has to change.
 */
export function renderLines(source, options = {}) {
  const settings = {
    theme: options.theme ?? "simple",
    color: options.color ?? true,
    terminal: options.terminal ?? null,
    hyperlinks: options.hyperlinks ?? true,
  };

  if (settings.theme === "desktop") {
    const trimmed = source.trim();
    const lines = rustLines(trimmed).map((line) => [
      { text: line, style: plainStyle() },
    ]);
    return { lines, sourceMap: lines.map((_, index) => index) };
  }

  const sourceLines = rustLines(source);
  const output = [];
  let owedBlank = false;
  let index = 0;

  const add = (runs, sourceIndex) => {
    let current = [];
    for (const run of runs) {
      const parts = run.text.split("\n");
      parts.forEach((part, offset) => {
        if (offset > 0) {
          output.push({ runs: current, source: sourceIndex, rule: false });
          current = [];
        }
        if (part !== "") {
          current.push({ text: part, style: run.style });
        }
      });
    }
    output.push({ runs: current, source: sourceIndex, rule: false });
  };
  const addBlank = (sourceIndex) => {
    const last = output[output.length - 1];
    if (!last || plainOf(last.runs) !== "") {
      add([], sourceIndex);
    }
  };
  const settled = (sourceIndex) => {
    if (owedBlank) {
      addBlank(sourceIndex);
      owedBlank = false;
    }
  };

  while (index < sourceLines.length) {
    const line = sourceLines[index];

    if (line.trim() === "") {
      add([], index);
      owedBlank = false;
      index += 1;
      continue;
    }

    const fence = fenceOpening(line);
    if (fence !== null) {
      settled(index);
      const taken = takeFence(sourceLines, index + 1, fence);
      if (taken.code !== "") {
        rustLines(taken.code).forEach((codeLine, offset) => {
          add(
            [
              {
                text: expandTabs(codeLine, CODE_TAB_WIDTH),
                style: plainStyle(),
              },
            ],
            Math.min(taken.start + offset, taken.next - 1),
          );
        });
      }
      index = taken.next;
      continue;
    }

    if (
      isRow(line) &&
      index + 1 < sourceLines.length &&
      isSeparator(sourceLines[index + 1])
    ) {
      settled(index);
      const table = renderTable(sourceLines, index, settings);
      for (const entry of table.output) {
        add(entry.runs, entry.source);
      }
      index = table.next;
      continue;
    }

    if (isRule(line)) {
      settled(index);
      output.push({ runs: [], source: index, rule: true });
      index += 1;
      continue;
    }

    const title = heading(line);
    if (title !== null) {
      settled(index);
      addBlank(index);
      const style = plainStyle();
      if (title.level <= 3) {
        style.bold = true;
      }
      add(inlineRender(title.text, style, settings), index);
      owedBlank = true;
      index += 1;
      continue;
    }

    if (line.startsWith(">")) {
      settled(index);
      const body = [];
      let next = index;
      while (next < sourceLines.length && sourceLines[next].startsWith(">")) {
        let rest = sourceLines[next].slice(1);
        if (rest.startsWith(" ")) {
          rest = rest.slice(1);
        }
        body.push({ text: rest, source: next });
        next += 1;
      }
      add(drawnQuote(body, settings), index);
      index = next;
      continue;
    }

    settled(index);
    add(inlineRender(line, plainStyle(), settings), index);
    index += 1;
  }

  const trimmed = output.map((entry) => ({
    ...entry,
    runs: trimRunsEnd(entry.runs),
  }));
  const contentWidth = Math.max(
    1,
    ...trimmed
      .filter((entry) => !entry.rule)
      .map((entry) => displayWidth(plainOf(entry.runs))),
  );
  const ruleWidth =
    settings.theme === "pretty" && settings.terminal !== null
      ? Math.max(1, settings.terminal)
      : contentWidth;
  const mark = settings.theme === "pretty" ? "\u2500" : "-";
  const grey = plainStyle();
  grey.fg = GREY;
  const drawn = trimmed.map((entry) =>
    entry.rule
      ? {
          ...entry,
          runs: [paint(mark.repeat(ruleWidth), grey, settings)],
        }
      : entry,
  );

  let start = drawn.findIndex((entry) => plainOf(entry.runs) !== "");
  if (start < 0) {
    start = drawn.length;
  }
  let end = drawn.length;
  while (end > start && plainOf(drawn[end - 1].runs) === "") {
    end -= 1;
  }
  const kept = drawn.slice(start, end);
  return {
    lines: kept.map((entry) => entry.runs),
    sourceMap: kept.map((entry) => entry.source),
  };
}

/** The rendered text, with no styles. */
export function renderPlain(source, options = {}) {
  return renderLines(source, options).lines.map(plainOf).join("\n");
}

export { plainOf };
