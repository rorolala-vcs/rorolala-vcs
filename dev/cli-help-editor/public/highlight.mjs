// The editor's own layer: the source exactly as it is written, with the markers
// dimmed and the text they hold drawn in the colour it asks for. The preview is
// the authority on the final shape; this only makes the source readable while it
// is typed.

import { rgbOf } from "/lib/render.mjs";

const DELIMITERS = ["***", "**", "__", "~~", "*"];

function escapeHtml(text) {
  return text
    .replace(/&/gu, "&amp;")
    .replace(/</gu, "&lt;")
    .replace(/>/gu, "&gt;");
}

function css(color, theme) {
  if (!color) {
    return null;
  }
  const rgb = rgbOf(color, theme);
  return rgb === null ? null : `rgb(${rgb[0]}, ${rgb[1]}, ${rgb[2]})`;
}

function styleAttributes(style, theme) {
  const found = [];
  if (style.color) {
    found.push(`color:${css(style.color, theme) ?? "inherit"}`);
  }
  if (style.background) {
    found.push(
      `background-color:${css(style.background, theme) ?? "transparent"}`,
    );
  }
  return found.length === 0 ? "" : ` style="${found.join(";")}"`;
}

function classes(style) {
  const found = [];
  if (style.bold) {
    found.push("hl-bold");
  }
  if (style.italic) {
    found.push("hl-italic");
  }
  if (style.underline) {
    found.push("hl-underline");
  }
  if (style.strike) {
    found.push("hl-strike");
  }
  if (style.code) {
    found.push("hl-code");
  }
  if (style.flag) {
    found.push("hl-flag");
  }
  return found.length === 0 ? "" : ` class="${found.join(" ")}"`;
}

function attributesOf(delimiter) {
  switch (delimiter) {
    case "***":
      return { bold: true, italic: true };
    case "**":
      return { bold: true };
    case "__":
      return { underline: true };
    case "~~":
      return { strike: true };
    default:
      return { italic: true };
  }
}

function namedColor(name) {
  const base = {
    black: 0,
    red: 1,
    green: 2,
    yellow: 3,
    blue: 4,
    magenta: 5,
    cyan: 6,
    white: 7,
  }[name.toLowerCase()];
  if (base === undefined) {
    return null;
  }
  const bright = name === name.toUpperCase() && name !== name.toLowerCase();
  return { kind: "named", index: bright ? base + 8 : base };
}

function exactColor(text) {
  if (!/^#[0-9a-fA-F]{6}$/u.test(text)) {
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

/** One source line, as spans that keep every character. */
export function highlightLine(line, theme = "simple") {
  const base = {
    bold: false,
    italic: false,
    underline: false,
    strike: false,
    code: false,
    flag: false,
    color: null,
    background: null,
  };
  let style = { ...base };
  const directives = [];
  let html = "";
  let at = 0;

  const emitter = (text, current, mark = false) => {
    if (text === "") {
      return;
    }
    const found = mark ? ' class="mark"' : classes(current);
    html += `<span${found}${styleAttributes(current, theme)}>${escapeHtml(text)}</span>`;
  };

  while (at < line.length) {
    const rest = line.slice(at);

    if (rest.startsWith("\\")) {
      const escaped = /^\\u(?:\{[0-9a-fA-F]+\}|[0-9a-fA-F]{4})/u.exec(rest);
      const length =
        escaped === null ? Math.min(2, rest.length) : escaped[0].length;
      emitter(rest.slice(0, length), style, true);
      at += length;
      continue;
    }

    if (rest.startsWith("[[")) {
      const end = rest.indexOf("]]");
      if (end >= 0) {
        const inner = rest.slice(2, end);
        emitter(rest.slice(0, end + 2), style, true);
        if (inner === "/") {
          const previous = directives.pop();
          if (previous !== undefined) {
            style = previous;
          }
        } else if (inner.startsWith("?")) {
          directives.push({ ...style });
          style = { ...style };
        } else if (inner.startsWith("~")) {
          const found = color(inner.slice(1));
          if (found !== null) {
            directives.push({ ...style });
            style = { ...style, background: found };
          }
        } else {
          const found = color(inner);
          if (found !== null) {
            directives.push({ ...style });
            style = { ...style, color: found };
          }
        }
        at += end + 2;
        continue;
      }
    }

    if (rest.startsWith("`")) {
      if (style.code || line.indexOf("`", at + 1) >= 0) {
        style = { ...style, code: !style.code };
        emitter("`", style, true);
        at += 1;
        continue;
      }
    }

    if (rest.startsWith(">") && style.flag) {
      style = { ...style, flag: false };
      emitter(">", style, true);
      at += 1;
      continue;
    }

    if (rest.startsWith("<") && line.indexOf(">", at + 1) >= 0) {
      style = { ...style, flag: true };
      emitter("<", style, true);
      at += 1;
      continue;
    }

    let delimiter = null;
    for (const candidate of DELIMITERS) {
      if (rest.startsWith(candidate)) {
        delimiter = candidate;
        break;
      }
    }
    if (delimiter !== null) {
      const attributes = attributesOf(delimiter);
      const active = Object.entries(attributes).every(
        ([name, value]) => !value || style[name],
      );
      if (active || line.indexOf(delimiter, at + delimiter.length) >= 0) {
        for (const [name, value] of Object.entries(attributes)) {
          style = { ...style, [name]: value ? !style[name] : style[name] };
        }
        emitter(delimiter, style, true);
        at += delimiter.length;
        continue;
      }
    }

    let end = at + 1;
    while (end < line.length) {
      const tail = line.slice(end);
      if (
        tail.startsWith("\\") ||
        tail.startsWith("[[") ||
        tail.startsWith("`") ||
        tail.startsWith("<") ||
        DELIMITERS.some((candidate) => tail.startsWith(candidate))
      ) {
        break;
      }
      end += 1;
    }
    const found = line.slice(at, end);
    emitter(found, style);
    at = end;
    if (found === "" && at < line.length) {
      emitter(line[at], style);
      at += 1;
    }
  }

  return html;
}

/** A whole value, with its line endings kept. */
export function highlightText(text, theme = "simple") {
  return text
    .split("\n")
    .map((line) => highlightLine(line, theme))
    .join("\n");
}
