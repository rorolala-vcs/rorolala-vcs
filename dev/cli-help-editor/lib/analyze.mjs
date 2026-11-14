// The checks over the copy.
//
// They are the ones the editor can answer without building the program: whether a
// help block's right column lines up, whether a line runs past eighty columns,
// whether the two languages name the same placeholders, and whether a value holds
// a symbol this project does not write. The renderer is the authority on the
// columns, so a check measures what the reader will see.

import { displayWidth, lineColumn } from "./width.mjs";
import { renderLines, plainOf } from "./render.mjs";

const CANDIDATE = /^(\s{4,})(\S.*?)( {2,})(\S.*)$/u;
const PLACEHOLDER = /%\{([^}]*)\}/gu;
const BANNED =
  /[\u2014\u2013\u2012\u2015\u2212\u2E3A\u2E3B\u2026\u00B7\u2022\u301C\uFF5E]/gu;

const SYMBOLS = {
  0x2014: "em dash",
  0x2013: "en dash",
  0x2012: "figure dash",
  0x2015: "horizontal bar",
  0x2212: "minus sign",
  0x2e3a: "two em dash",
  0x2e3b: "three em dash",
  0x2026: "ellipsis",
  0x00b7: "middle dot",
  0x2022: "bullet",
  0x301c: "wave dash",
  0xff5e: "fullwidth tilde",
};

function finding(key, lang, kind, severity, message, extra = {}) {
  return {
    key: key.id,
    lang,
    kind,
    severity,
    message,
    ...extra,
  };
}

function sourceLine(node, valueLine) {
  const base = node.style === "block" ? node.line + 1 : node.line;
  return base + valueLine;
}

function valueLine(value, valueLineIndex) {
  return value.split("\n")[valueLineIndex] ?? "";
}

function placeholders(value) {
  const found = [];
  for (const match of value.matchAll(PLACEHOLDER)) {
    found.push({
      name: match[1],
      offset: match.index,
      length: match[0].length,
    });
  }
  return found;
}

function masked(value) {
  const lines = value.split("\n");
  let fence = null;
  return lines
    .map((line) => {
      if (fence === null) {
        const opening = /^[ \t]*(`{3,}|~{3,})/.exec(line);
        if (opening !== null) {
          fence = opening[1][0];
          return " ".repeat(line.length);
        }
      } else {
        const closing = /^[ \t]*(`{3,}|~{3,})[ \t]*$/.exec(line);
        if (closing !== null && closing[1][0] === fence) {
          fence = null;
        }
        return " ".repeat(line.length);
      }
      return line
        .replace(/`[^`]*`/gu, (found) => " ".repeat(found.length))
        .replace(/\[\[[^\]]*\]\]/gu, (found) => " ".repeat(found.length));
    })
    .join("\n");
}

function banned(value) {
  const found = [];
  const visible = masked(value);
  for (const match of visible.matchAll(BANNED)) {
    const symbol = SYMBOLS[match[0].codePointAt(0)] ?? "banned symbol";
    found.push({
      offset: match.index,
      length: match[0].length,
      symbol,
      codePoint: match[0].codePointAt(0),
    });
  }
  return found;
}

function openDirectives(value) {
  const found = [];
  const lines = value.split("\n");
  let offset = 0;
  let fence = null;
  for (const line of lines) {
    if (fence === null) {
      const opening = /^[ \t]*(`{3,}|~{3,})/.exec(line);
      if (opening !== null) {
        fence = opening[1][0];
        offset += line.length + 1;
        continue;
      }
    } else {
      const closing = /^[ \t]*(`{3,}|~{3,})[ \t]*$/.exec(line);
      if (closing !== null && closing[1][0] === fence) {
        fence = null;
      }
      offset += line.length + 1;
      continue;
    }

    let at = 0;
    for (;;) {
      const start = line.indexOf("[[", at);
      if (start < 0) {
        break;
      }
      const end = line.indexOf("]]", start + 2);
      if (end < 0) {
        found.push({ offset: offset + start, length: 2 });
        at = start + 2;
      } else {
        at = end + 2;
      }
    }
    offset += line.length + 1;
  }
  return found;
}

function oddBackticks(value) {
  let count = 0;
  for (const character of masked(value)) {
    if (character === "`") {
      count += 1;
    }
  }
  return count % 2 === 1;
}

function checkAlignment(key, lang, node, value, maxWidth) {
  const findings = [];
  const rendered = renderLines(value, { theme: "text", color: false });
  const lines = rendered.lines.map(plainOf);
  const rows = [];

  for (let index = 0; index < lines.length; index += 1) {
    const width = displayWidth(lines[index]);
    if (width > maxWidth) {
      const at = rendered.sourceMap[index] ?? 0;
      findings.push(
        finding(key, lang, "overlong", "info", `over ${maxWidth} columns`, {
          line: sourceLine(node, at),
          text: lines[index],
          width,
        }),
      );
    }
  }

  let group = [];
  const flush = () => {
    if (group.length >= 2) {
      const counts = new Map();
      for (const row of group) {
        counts.set(row.right, (counts.get(row.right) ?? 0) + 1);
      }
      let target = 0;
      let most = -1;
      for (const [right, count] of counts) {
        if (count > most || (count === most && right > target)) {
          target = right;
          most = count;
        }
      }
      for (const row of group) {
        if (row.right !== target) {
          findings.push(
            finding(
              key,
              lang,
              "alignment",
              "warn",
              `right column at ${row.right}, not ${target}`,
              {
                line: sourceLine(node, rendered.sourceMap[row.index] ?? 0),
                text: lines[row.index],
              },
            ),
          );
        }
      }
    }
    group = [];
  };

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim() === "") {
      flush();
      continue;
    }
    const match = CANDIDATE.exec(line);
    if (match === null) {
      flush();
      continue;
    }
    const left = `${match[1]}${match[2]}`;
    const gap = match[3].length;
    const row = {
      index,
      leftEnd: displayWidth(left),
      right: displayWidth(left) + gap,
    };
    group.push(row);
    rows.push(row);
  }
  flush();

  if (rows.length >= 2) {
    const counts = new Map();
    for (const row of rows) {
      counts.set(row.right, (counts.get(row.right) ?? 0) + 1);
    }
    let anchor = 0;
    let most = -1;
    for (const [right, count] of counts) {
      if (count > most || (count === most && right > anchor)) {
        anchor = right;
        most = count;
      }
    }

    const aligned = rows.filter((row) => row.right === anchor);
    const widest = Math.max(...aligned.map((row) => row.leftEnd));
    const gap = anchor - widest;
    const row =
      aligned.find((candidate) => candidate.leftEnd === widest) ?? rows[0];
    if (gap < 2) {
      findings.push(
        finding(
          key,
          lang,
          "alignment",
          "warn",
          "right column overlaps the widest left cell",
          {
            line: sourceLine(node, rendered.sourceMap[row.index] ?? 0),
            text: lines[row.index],
          },
        ),
      );
    } else if (gap !== 4) {
      findings.push(
        finding(key, lang, "alignment", "info", `gap ${gap}, not 4`, {
          line: sourceLine(node, rendered.sourceMap[row.index] ?? 0),
          text: lines[row.index],
        }),
      );
    }
  }

  return findings;
}

function checkLanguage(key, lang, options) {
  const node = key.langs[lang];
  if (node === undefined) {
    return [finding(key, lang, "missing", "warn", `${lang} is missing`)];
  }

  const value = node.value;
  const findings = [];

  // An empty draft is where a draft starts, not a message missing its words.
  if (value.trim() === "" && key.kind !== "draft") {
    findings.push(
      finding(key, lang, "empty", "warn", "value is empty", {
        line: sourceLine(node, 0),
        text: "",
        offset: 0,
        length: 0,
      }),
    );
  }

  for (const found of placeholders(value)) {
    if (found.name === "") {
      const at = lineColumn(value, found.offset);
      findings.push(
        finding(key, lang, "placeholder", "warn", "empty placeholder", {
          line: sourceLine(node, at.line),
          offset: found.offset,
          length: found.length,
          text: valueLine(value, at.line),
        }),
      );
    }
  }

  for (const found of banned(value)) {
    const at = lineColumn(value, found.offset);
    findings.push(
      finding(
        key,
        lang,
        "copy",
        "info",
        `U+${found.codePoint.toString(16).toUpperCase().padStart(4, "0")} ${found.symbol}`,
        {
          line: sourceLine(node, at.line),
          offset: found.offset,
          length: found.length,
          text: valueLine(value, at.line),
        },
      ),
    );
  }

  for (const found of openDirectives(value)) {
    const at = lineColumn(value, found.offset);
    findings.push(
      finding(key, lang, "markup", "info", "unclosed [[", {
        line: sourceLine(node, at.line),
        offset: found.offset,
        length: found.length,
        text: valueLine(value, at.line),
      }),
    );
  }

  if (oddBackticks(value)) {
    findings.push(
      finding(key, lang, "markup", "info", "odd number of backticks", {
        line: sourceLine(node, 0),
        text: value.split("\n")[0] ?? "",
      }),
    );
  }

  if (key.kind === "help" || key.kind === "draft") {
    findings.push(...checkAlignment(key, lang, node, value, options.maxWidth));
  }

  return findings;
}

/** Every finding one key has. */
export function analyzeKey(key, options = {}) {
  const settings = {
    maxWidth: options.maxWidth ?? 80,
  };
  const findings = [];

  // A draft is one text rather than a pair, so the languages it holds are the ones
  // checked; a key of the CLI is always written in both.
  const languages =
    key.kind === "draft" ? Object.keys(key.langs) : ["en", "zh-CN"];
  for (const lang of languages) {
    findings.push(...checkLanguage(key, lang, settings));
  }

  const en = key.langs.en;
  const zh = key.langs["zh-CN"];

  if (en !== undefined && zh !== undefined) {
    const english = new Map(
      placeholders(en.value).map((found) => [found.name, found]),
    );
    const chinese = new Map(
      placeholders(zh.value).map((found) => [found.name, found]),
    );
    for (const [name, found] of english) {
      if (!chinese.has(name)) {
        const at = lineColumn(en.value, found.offset);
        findings.push(
          finding(key, "en", "placeholder", "warn", `zh-CN lacks %{${name}}`, {
            line: sourceLine(en, at.line),
            offset: found.offset,
            length: found.length,
            text: valueLine(en.value, at.line),
          }),
        );
      }
    }
    for (const [name, found] of chinese) {
      if (!english.has(name)) {
        const at = lineColumn(zh.value, found.offset);
        findings.push(
          finding(key, "zh-CN", "placeholder", "warn", `en lacks %{${name}}`, {
            line: sourceLine(zh, at.line),
            offset: found.offset,
            length: found.length,
            text: valueLine(zh.value, at.line),
          }),
        );
      }
    }
  }

  return findings;
}

/** Every finding in the tree, the ones a reader should look at first first. */
export function analyzeAll(keys, options = {}) {
  const order = { warn: 0, info: 1 };
  return keys
    .flatMap((key) => analyzeKey(key, options))
    .sort((left, right) => {
      const severity =
        (order[left.severity] ?? 2) - (order[right.severity] ?? 2);
      if (severity !== 0) {
        return severity;
      }
      if (left.key !== right.key) {
        return left.key.localeCompare(right.key);
      }
      if (left.lang !== right.lang) {
        return left.lang.localeCompare(right.lang);
      }
      return (left.line ?? 0) - (right.line ?? 0);
    });
}

/** How many findings there are, by severity and by check. */
export function summarize(findings) {
  const summary = {
    total: findings.length,
    warn: 0,
    info: 0,
    kinds: {},
  };
  for (const found of findings) {
    summary[found.severity] = (summary[found.severity] ?? 0) + 1;
    summary.kinds[found.kind] = (summary.kinds[found.kind] ?? 0) + 1;
  }
  return summary;
}
