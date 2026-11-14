// The subset of YAML the i18n files are written in, with the byte positions an
// editor needs.
//
// The translation files are not arbitrary YAML. They are maps of identifiers,
// with `en` and `zh-CN` leaves that are a literal block, a double quoted string
// or a plain scalar. Reading them with a general parser and writing them back
// through a dumper would lose the comments, the blank lines and the hand-made
// alignment, so this reads exactly that subset and keeps the source lines around.
// A construct outside it is marked unsupported instead of guessed at.

const KEY_ONLY = /^([^\s:#][^:]*?):[ \t]*$/;
const KEY_VALUE = /^([^\s:#][^:]*?):([ \t]+)(.*)$/;
const LANGUAGE = new Set(["en", "zh-CN"]);

/** Splits `text` into lines, keeping the line ending the file uses. */
export function splitSource(source) {
  const bom = source.startsWith("\uFEFF") ? "\uFEFF" : "";
  const body = bom === "" ? source : source.slice(1);
  const eol = body.includes("\r\n") ? "\r\n" : "\n";
  const lines = body
    .split("\n")
    .map((line) => (line.endsWith("\r") ? line.slice(0, -1) : line));
  return { bom, eol, lines };
}

/** Joins lines back into a source, with the line ending it was read with. */
export function joinSource(file) {
  return `${file.bom}${file.lines.join(file.eol)}`;
}

function decodeQuoted(raw) {
  let value = "";
  let at = 0;
  while (at < raw.length) {
    const character = raw[at];
    if (character !== "\\") {
      value += character;
      at += 1;
      continue;
    }
    const escape = raw[at + 1];
    at += 2;
    switch (escape) {
      case "0":
        value += "\0";
        break;
      case "a":
        value += "\u0007";
        break;
      case "b":
        value += "\b";
        break;
      case "t":
        value += "\t";
        break;
      case "n":
        value += "\n";
        break;
      case "v":
        value += "\u000b";
        break;
      case "f":
        value += "\f";
        break;
      case "r":
        value += "\r";
        break;
      case "e":
        value += "\u001b";
        break;
      case " ":
        value += " ";
        break;
      case '"':
        value += '"';
        break;
      case "/":
        value += "/";
        break;
      case "\\":
        value += "\\";
        break;
      case "N":
        value += "\u0085";
        break;
      case "_":
        value += "\u00a0";
        break;
      case "L":
        value += "\u2028";
        break;
      case "P":
        value += "\u2029";
        break;
      case "x":
      case "u":
      case "U": {
        const length = escape === "x" ? 2 : escape === "u" ? 4 : 8;
        const digits = raw.slice(at, at + length);
        if (/^[0-9a-fA-F]+$/.test(digits) && digits.length === length) {
          value += String.fromCodePoint(Number.parseInt(digits, 16));
          at += length;
        } else {
          value += `\\${escape}`;
        }
        break;
      }
      default:
        value += `\\${escape ?? ""}`;
        break;
    }
  }
  return value;
}

function encodeQuoted(value) {
  let encoded = "";
  for (const character of value) {
    switch (character) {
      case "\\":
        encoded += "\\\\";
        break;
      case '"':
        encoded += '\\"';
        break;
      case "\n":
        encoded += "\\n";
        break;
      case "\r":
        encoded += "\\r";
        break;
      case "\t":
        encoded += "\\t";
        break;
      case "\b":
        encoded += "\\b";
        break;
      case "\f":
        encoded += "\\f";
        break;
      default: {
        const code = character.codePointAt(0);
        if (code < 0x20 || code === 0x7f) {
          encoded += `\\x${code.toString(16).padStart(2, "0")}`;
        } else if (code === 0x85) {
          encoded += "\\N";
        } else if (code === 0x2028) {
          encoded += "\\L";
        } else if (code === 0x2029) {
          encoded += "\\P";
        } else {
          encoded += character;
        }
      }
    }
  }
  return encoded;
}

function safePlain(value) {
  if (value === "" || /[\n\r\t]/.test(value)) {
    return false;
  }
  if (/^[-?:,[\]{}#&*!|>'"%@` ]/.test(value)) {
    return false;
  }
  if (value.includes(": ") || value.includes(" #") || value.endsWith(":")) {
    return false;
  }
  return true;
}

function clippedBlock(content) {
  return content.map((entry) => entry.text).join("\n");
}

/**
 * Reads one i18n file.
 *
 * `nodes` are the `en` and `zh-CN` leaves; `problems` name a construct this
 * reader does not edit, so the UI can leave it alone instead of corrupting it.
 */
export function parseFile(source, file, languages = LANGUAGE) {
  const split = splitSource(source);
  const lines = split.lines;
  const nodes = [];
  const problems = [];
  const stack = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];
    if (line.trim() === "" || line.trimStart().startsWith("#")) {
      index += 1;
      continue;
    }

    const indent = line.length - line.trimStart().length;
    const slice = line.slice(indent);
    const keyOnly = KEY_ONLY.exec(slice);
    const keyValue = KEY_VALUE.exec(slice);
    const match = keyOnly ?? keyValue;
    if (match === null) {
      index += 1;
      continue;
    }

    const key = match[1];
    const rest = keyValue === null ? "" : keyValue[3];
    const spaces = keyValue === null ? 0 : keyValue[2].length;
    const restStart = indent + key.length + 1 + spaces;

    while (stack.length > 0 && stack[stack.length - 1].indent >= indent) {
      stack.pop();
    }

    if (keyOnly !== null) {
      if (languages.has(key)) {
        const parent = stack.map((entry) => entry.key);
        nodes.push({
          file,
          id: parent.join("."),
          path: parent,
          lang: key,
          style: "unsupported",
          value: "",
          keyLine: index,
          keyIndent: indent,
          editable: false,
        });
        problems.push({
          file,
          line: index + 1,
          message: "empty scalar is not edited",
        });
      } else {
        stack.push({ indent, key });
      }
      index += 1;
      continue;
    }

    if (languages.has(key)) {
      const parent = stack.map((entry) => entry.key);
      const id = parent.join(".");
      if (rest === "") {
        problems.push({
          file,
          line: index + 1,
          message: "empty scalar",
        });
      } else if (rest === "|" || rest === "|-" || rest === "|+") {
        let next = index + 1;
        let lastNonBlank = index;
        let detectedIndent = null;
        const content = [];
        while (next < lines.length) {
          const candidate = lines[next];
          if (candidate.trim() === "") {
            content.push({ line: next, text: "" });
            next += 1;
            continue;
          }
          const candidateIndent =
            candidate.length - candidate.trimStart().length;
          if (candidateIndent <= indent) {
            break;
          }
          if (detectedIndent === null) {
            detectedIndent = candidateIndent;
          }
          content.push({
            line: next,
            text: candidate.slice(Math.min(detectedIndent, candidateIndent)),
          });
          lastNonBlank = next;
          next += 1;
        }
        const kept = content.filter((entry) => entry.line <= lastNonBlank);
        nodes.push({
          file,
          id,
          path: parent,
          lang: key,
          style: "block",
          indicator: rest,
          value: clippedBlock(kept),
          keyLine: index,
          keyIndent: indent,
          blockStart: index + 1,
          blockEnd: lastNonBlank + 1,
          contentIndent: detectedIndent ?? indent + 4,
          editable: true,
        });
        index = lastNonBlank + 1;
        continue;
      } else if (rest === ">" || rest === ">-" || rest === ">+") {
        let next = index + 1;
        while (next < lines.length) {
          if (lines[next].trim() === "") {
            next += 1;
            continue;
          }
          const candidateIndent =
            lines[next].length - lines[next].trimStart().length;
          if (candidateIndent <= indent) {
            break;
          }
          next += 1;
        }
        nodes.push({
          file,
          id,
          path: parent,
          lang: key,
          style: "unsupported",
          indicator: rest,
          value: "",
          keyLine: index,
          keyIndent: indent,
          editable: false,
        });
        problems.push({
          file,
          line: index + 1,
          message: "folded scalar is not edited",
        });
        index = next;
        continue;
      } else if (rest.startsWith('"')) {
        let at = 1;
        while (at < rest.length) {
          if (rest[at] === "\\") {
            at += 2;
            continue;
          }
          if (rest[at] === '"') {
            break;
          }
          at += 1;
        }
        if (at >= rest.length) {
          let next = index + 1;
          while (next < lines.length) {
            if (lines[next].trim() === "") {
              next += 1;
              continue;
            }
            const candidateIndent =
              lines[next].length - lines[next].trimStart().length;
            if (candidateIndent <= indent) {
              break;
            }
            next += 1;
          }
          nodes.push({
            file,
            id,
            path: parent,
            lang: key,
            style: "unsupported",
            indicator: rest,
            value: "",
            keyLine: index,
            keyIndent: indent,
            editable: false,
          });
          problems.push({
            file,
            line: index + 1,
            message: "quoted scalar does not close on its line",
          });
          index = next;
          continue;
        } else {
          nodes.push({
            file,
            id,
            path: parent,
            lang: key,
            style: "quoted",
            value: decodeQuoted(rest.slice(1, at)),
            keyLine: index,
            keyIndent: indent,
            valueStart: restStart,
            valueEnd: restStart + at + 1,
            editable: true,
          });
        }
      } else if (rest.startsWith("'")) {
        nodes.push({
          file,
          id,
          path: parent,
          lang: key,
          style: "unsupported",
          indicator: rest,
          value: rest.slice(1, -1),
          keyLine: index,
          keyIndent: indent,
          editable: false,
        });
        problems.push({
          file,
          line: index + 1,
          message: "single quoted scalar is not edited",
        });
      } else {
        const value = rest.replace(/\s+$/u, "");
        nodes.push({
          file,
          id,
          path: parent,
          lang: key,
          style: "plain",
          value,
          keyLine: index,
          keyIndent: indent,
          valueStart: restStart,
          valueEnd: restStart + value.length,
          editable: true,
        });
      }
      index += 1;
      continue;
    }

    index += 1;
  }

  return { file, ...split, nodes, problems };
}

/**
 * Writes `value` over one node, in place.
 *
 * The lines are the file's, already split. A block keeps its own indentation and
 * its own style. A scalar that no longer fits a plain value becomes quoted, and
 * a scalar that holds a newline becomes a quoted string with `\n`.
 */
export function applyEdit(lines, node, value) {
  if (!node.editable) {
    return false;
  }

  if (node.style === "block") {
    const content = value === "" ? [] : value.split("\n");
    const indent = " ".repeat(node.contentIndent);
    const inserted = content.map((line) => (line === "" ? "" : indent + line));
    lines.splice(node.blockStart, node.blockEnd - node.blockStart, ...inserted);
    return true;
  }

  const line = lines[node.keyLine];
  if (line === undefined) {
    return false;
  }

  if (node.style === "quoted") {
    const encoded = `"${encodeQuoted(value)}"`;
    lines[node.keyLine] =
      line.slice(0, node.valueStart) + encoded + line.slice(node.valueEnd);
    return true;
  }

  if (node.style === "plain") {
    const replacement = safePlain(value) ? value : `"${encodeQuoted(value)}"`;
    lines[node.keyLine] =
      line.slice(0, node.valueStart) + replacement + line.slice(node.valueEnd);
    return true;
  }

  return false;
}

export { encodeQuoted, safePlain };
