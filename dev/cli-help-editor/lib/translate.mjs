// The translation endpoint the editor talks to.
//
// The browser never holds the API key and never calls the provider directly. It
// posts the source text to the local server, which reads the TOML configuration
// once per request, asks an OpenAI compatible endpoint for a stream, and passes
// the text deltas back as server sent events.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { EditorError, writeAtomic } from "./scan.mjs";

const LANGUAGES = {
  en: "English",
  "zh-CN": "Simplified Chinese",
};

/** The candidate paths of the tool's configuration, most preferred first. */
export function configCandidates() {
  const configured = process.env.ROLA_DATA_HOME;
  if (configured) {
    return [
      path.join(path.resolve(configured), "rola", "dev", "llm-tools.toml"),
    ];
  }

  const homes = [];
  if (process.platform === "win32") {
    homes.push(
      process.env.LOCALAPPDATA ?? path.join(os.homedir(), "AppData", "Local"),
    );
  } else if (process.platform === "darwin") {
    homes.push(path.join(os.homedir(), "Library", "Application Support"));
  } else {
    homes.push(
      process.env.XDG_DATA_HOME || path.join(os.homedir(), ".local", "share"),
    );
  }
  homes.push(path.join(os.homedir(), ".local", "share"));

  return [...new Set(homes.map((home) => path.resolve(home)))].map((home) =>
    path.join(home, "rola", "dev", "llm-tools.toml"),
  );
}

function findStringEnd(text, start) {
  for (let index = start + 1; index < text.length; index += 1) {
    if (text[index] === "\\") {
      index += 1;
      continue;
    }
    if (text[index] === '"') {
      return index;
    }
  }
  return -1;
}

function decodeBasicString(raw, line) {
  let value = "";
  for (let index = 1; index < raw.length - 1; index += 1) {
    const character = raw[index];
    if (character !== "\\") {
      value += character;
      continue;
    }
    index += 1;
    const escape = raw[index];
    switch (escape) {
      case "b":
        value += "\b";
        break;
      case "t":
        value += "\t";
        break;
      case "n":
        value += "\n";
        break;
      case "f":
        value += "\f";
        break;
      case "r":
        value += "\r";
        break;
      case '"':
        value += '"';
        break;
      case "\\":
        value += "\\";
        break;
      case "u":
      case "U": {
        const length = escape === "u" ? 4 : 8;
        const digits = raw.slice(index + 1, index + 1 + length);
        if (!new RegExp(`^[0-9a-fA-F]{${length}}$`, "u").test(digits)) {
          throw new Error(`line ${line}: bad unicode escape`);
        }
        value += String.fromCodePoint(Number.parseInt(digits, 16));
        index += length;
        break;
      }
      default:
        throw new Error(`line ${line}: unknown escape \\${escape}`);
    }
  }
  return value;
}

function literalString(raw, line) {
  const end = raw.indexOf("'", 1);
  if (end < 0) {
    throw new Error(`line ${line}: unterminated string`);
  }
  if (raw.slice(end + 1).trim() !== "") {
    throw new Error(`line ${line}: text after the string`);
  }
  return raw.slice(1, end);
}

function basicString(raw, line) {
  const end = findStringEnd(raw, 0);
  if (end < 0) {
    throw new Error(`line ${line}: unterminated string`);
  }
  if (raw.slice(end + 1).trim() !== "") {
    throw new Error(`line ${line}: text after the string`);
  }
  return decodeBasicString(raw.slice(0, end + 1), line);
}

function parseValue(raw, line) {
  if (raw.startsWith('"')) {
    return basicString(raw, line);
  }
  if (raw.startsWith("'")) {
    return literalString(raw, line);
  }
  if (raw === "true") {
    return true;
  }
  if (raw === "false") {
    return false;
  }
  if (/^[+-]?\d+(?:\.\d+)?$/u.test(raw)) {
    return Number(raw);
  }
  throw new Error(`line ${line}: unsupported value`);
}

function parseKey(raw, line) {
  if (raw.startsWith('"') || raw.startsWith("'")) {
    return parseValue(raw, line);
  }
  if (!/^[A-Za-z0-9_-]+$/u.test(raw)) {
    throw new Error(`line ${line}: unsupported key`);
  }
  return raw;
}

function stripComment(line) {
  let quote = null;
  let escaped = false;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (quote === '"') {
      if (escaped) {
        escaped = false;
      } else if (character === "\\") {
        escaped = true;
      } else if (character === '"') {
        quote = null;
      }
      continue;
    }
    if (quote === "'") {
      if (character === "'") {
        quote = null;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      continue;
    }
    if (character === "#") {
      return line.slice(0, index);
    }
  }
  return line;
}

/**
 * The subset of TOML the tool's configuration uses.
 *
 * Tables, bare or quoted keys, basic and literal strings, numbers, booleans and
 * comments. It reports the line it could not read, so the error names a place.
 */
export function parseToml(text) {
  const root = {};
  let current = root;
  const body = text.startsWith("\uFEFF") ? text.slice(1) : text;
  const lines = body.split(/\r?\n/u);

  lines.forEach((raw, index) => {
    const line = stripComment(raw).trim();
    const number = index + 1;
    if (line === "") {
      return;
    }
    if (line.startsWith("[") && line.endsWith("]")) {
      const name = line.slice(1, -1).trim();
      if (name === "") {
        throw new Error(`line ${number}: empty table name`);
      }
      current = root;
      for (const part of name.split(".")) {
        const key = parseKey(part.trim(), number);
        if (current[key] === undefined) {
          current[key] = {};
        }
        if (typeof current[key] !== "object" || current[key] === null) {
          throw new Error(`line ${number}: ${key} is not a table`);
        }
        current = current[key];
      }
      return;
    }
    const equals = line.indexOf("=");
    if (equals < 0) {
      throw new Error(`line ${number}: expected key = value`);
    }
    const key = parseKey(line.slice(0, equals).trim(), number);
    current[key] = parseValue(line.slice(equals + 1).trim(), number);
  });

  return root;
}

/** The endpoint a base URL names. */
export function endpoint(baseUrl) {
  const trimmed = baseUrl.trim().replace(/\/+$/u, "");
  return trimmed.endsWith("/chat/completions")
    ? trimmed
    : `${trimmed}/chat/completions`;
}

const DEFAULT_TEMPERATURE = 0.2;
const DEFAULT_TIMEOUT = 120;
const MAX_TIMEOUT = 600;

/** The file the settings live in, and whether it is there yet. */
export function settingsFile(candidates = configCandidates()) {
  const existing = candidates.find((candidate) => fs.existsSync(candidate));
  return { file: existing ?? candidates[0], exists: existing !== undefined };
}

/** The settings an absent file stands for. */
function defaults() {
  return {
    baseUrl: "",
    model: "",
    apiKey: "",
    temperature: DEFAULT_TEMPERATURE,
    timeout: DEFAULT_TIMEOUT,
    headers: {},
  };
}

function configOf(section) {
  const baseUrl =
    typeof section.base_url === "string" ? section.base_url.trim() : "";
  const model = typeof section.model === "string" ? section.model.trim() : "";
  const apiKey = typeof section.api_key === "string" ? section.api_key : "";
  const temperature =
    typeof section.temperature === "number" &&
    Number.isFinite(section.temperature)
      ? section.temperature
      : DEFAULT_TEMPERATURE;
  const timeout =
    typeof section.timeout === "number" &&
    Number.isFinite(section.timeout) &&
    section.timeout > 0
      ? Math.min(section.timeout, MAX_TIMEOUT)
      : DEFAULT_TIMEOUT;
  const headers = {};
  if (
    typeof section.headers === "object" &&
    section.headers !== null &&
    !Array.isArray(section.headers)
  ) {
    for (const [key, value] of Object.entries(section.headers)) {
      if (typeof value === "string") {
        headers[key] = value;
      }
    }
  }

  return { baseUrl, model, apiKey, temperature, timeout, headers };
}

/**
 * The settings as the editor shows them.
 *
 * A file that cannot be read is reported rather than thrown, so the editor can
 * open on the form the file would have and name the line that is wrong.
 */
export function readSettings(candidates = configCandidates()) {
  const { file, exists } = settingsFile(candidates);
  const settings = { file, exists, error: null, config: defaults() };
  if (!exists) {
    return settings;
  }

  let text;
  try {
    text = fs.readFileSync(file, "utf8");
  } catch (error) {
    settings.error = `${file}: ${error.message}`;
    return settings;
  }

  let parsed;
  try {
    parsed = parseToml(text);
  } catch (error) {
    settings.error = `${file}: ${error.message}`;
    return settings;
  }

  const section = parsed.translation ?? parsed;
  if (typeof section !== "object" || section === null) {
    settings.error = `${file}: the [translation] table is missing`;
    return settings;
  }

  settings.config = configOf(section);
  return settings;
}

/** The configuration, read from the first candidate that exists. */
export function readTranslationConfig(candidates = configCandidates()) {
  const settings = readSettings(candidates);
  if (!settings.exists) {
    throw new EditorError(
      `No [translation] endpoint in ${candidates.join(" or ")}`,
      503,
    );
  }
  if (settings.error !== null) {
    throw new EditorError(settings.error, 503);
  }

  const config = settings.config;
  if (config.baseUrl === "") {
    throw new EditorError(
      `${settings.file}: [translation].base_url is missing`,
      503,
    );
  }
  if (!/^https?:\/\//iu.test(config.baseUrl)) {
    throw new EditorError(
      `${settings.file}: [translation].base_url must start with http:// or https://`,
      503,
    );
  }
  if (config.model === "") {
    throw new EditorError(
      `${settings.file}: [translation].model is missing`,
      503,
    );
  }

  return { file: settings.file, ...config };
}

function quote(value) {
  let text = '"';
  for (const character of value) {
    const code = character.codePointAt(0);
    if (character === '"') {
      text += '\\"';
    } else if (character === "\\") {
      text += "\\\\";
    } else if (character === "\n") {
      text += "\\n";
    } else if (character === "\r") {
      text += "\\r";
    } else if (character === "\t") {
      text += "\\t";
    } else if (code < 0x20 || code === 0x7f) {
      text += `\\u${code.toString(16).padStart(4, "0")}`;
    } else {
      text += character;
    }
  }
  return `${text}"`;
}

function keyName(name) {
  return /^[A-Za-z0-9_-]+$/u.test(name) ? name : quote(name);
}

/** The `[translation]` section the editor form stands for. */
export function renderSettingsSection(config) {
  const lines = [
    "[translation]",
    `base_url = ${quote(config.baseUrl)}`,
    `api_key = ${quote(config.apiKey)}`,
    `model = ${quote(config.model)}`,
  ];
  if (config.temperature !== null) {
    lines.push(`temperature = ${String(config.temperature)}`);
  }
  if (config.timeout !== null) {
    lines.push(`timeout = ${String(config.timeout)}`);
  }

  const headers = Object.entries(config.headers);
  if (headers.length > 0) {
    lines.push("", "[translation.headers]");
    for (const [name, value] of headers) {
      lines.push(`${keyName(name)} = ${quote(value)}`);
    }
  }

  return lines;
}

const HEADER = /^\[([^\]]*)\]/u;

function headerName(line) {
  const match = HEADER.exec(stripComment(line).trim());
  return match === null ? null : match[1].trim();
}

/**
 * Puts a section back into a file, leaving everything outside it as it was.
 *
 * The `[translation]` table and its `[translation.*]` children are replaced in
 * one piece; other tables, their comments and the file's line endings are kept.
 */
export function replaceSection(text, name, block) {
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  const bom = text.startsWith("\uFEFF") ? "\uFEFF" : "";
  const body = bom === "" ? text : text.slice(1);
  const lines = body.split(/\r?\n/u);

  const head = [];
  let start = -1;
  for (let index = 0; index < lines.length; index += 1) {
    if (headerName(lines[index]) === name) {
      start = index;
      break;
    }
    head.push(lines[index]);
  }

  let tail = [];
  if (start >= 0) {
    let end = lines.length;
    for (let index = start + 1; index < lines.length; index += 1) {
      const header = headerName(lines[index]);
      if (
        header !== null &&
        header !== name &&
        !header.startsWith(`${name}.`)
      ) {
        end = index;
        break;
      }
    }
    tail = lines.slice(end);
  }

  while (head.length > 0 && head[head.length - 1].trim() === "") {
    head.pop();
  }
  while (tail.length > 0 && tail[0].trim() === "") {
    tail.shift();
  }

  const out = [];
  if (head.length > 0) {
    out.push(...head, "");
  }
  out.push(...block);
  if (tail.length > 0) {
    out.push("", ...tail);
  }
  while (out.length > 0 && out[out.length - 1] === "") {
    out.pop();
  }

  return `${bom}${out.join(eol)}${eol}`;
}

function validateSettings(input) {
  if (input === null || typeof input !== "object") {
    throw new EditorError("The settings are not an object");
  }

  const baseUrl = typeof input.baseUrl === "string" ? input.baseUrl.trim() : "";
  if (!/^https?:\/\//iu.test(baseUrl)) {
    throw new EditorError("base_url must start with http:// or https://");
  }

  const model = typeof input.model === "string" ? input.model.trim() : "";
  if (model === "") {
    throw new EditorError("model is required");
  }

  const apiKey = typeof input.apiKey === "string" ? input.apiKey.trim() : "";

  const temperature = input.temperature ?? null;
  if (
    temperature !== null &&
    (typeof temperature !== "number" ||
      !Number.isFinite(temperature) ||
      temperature < 0 ||
      temperature > 2)
  ) {
    throw new EditorError("temperature is a number from 0 to 2");
  }

  const timeout = input.timeout ?? null;
  if (
    timeout !== null &&
    (!Number.isInteger(timeout) || timeout < 1 || timeout > MAX_TIMEOUT)
  ) {
    throw new EditorError(
      `timeout is a whole number of seconds from 1 to ${MAX_TIMEOUT}`,
    );
  }

  const headers = {};
  if (input.headers !== undefined && input.headers !== null) {
    if (typeof input.headers !== "object" || Array.isArray(input.headers)) {
      throw new EditorError("headers is a table of names and values");
    }
    for (const [raw, value] of Object.entries(input.headers)) {
      const name = raw.trim();
      if (name === "") {
        throw new EditorError("a header has no name");
      }
      if (typeof value !== "string") {
        throw new EditorError(`the header ${name} is not text`);
      }
      headers[name] = value;
    }
  }

  return { baseUrl, model, apiKey, temperature, timeout, headers };
}

/**
 * Writes the configuration the editor form holds, then reads it back.
 *
 * The file is the one the translation endpoint is read from, so the editor and
 * the request cannot disagree about which configuration is in force.
 */
export function writeSettings(input, candidates = configCandidates()) {
  const config = validateSettings(input);
  const { file } = settingsFile(candidates);

  let text = "";
  if (fs.existsSync(file)) {
    try {
      text = fs.readFileSync(file, "utf8");
    } catch (error) {
      throw new EditorError(`${file}: ${error.message}`, 503);
    }
  }

  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
  } catch (error) {
    throw new EditorError(`${path.dirname(file)}: ${error.message}`, 500);
  }

  writeAtomic(
    file,
    replaceSection(text, "translation", renderSettingsSection(config)),
  );
  return readSettings(candidates);
}

function promptFor({ key, program, source, target }) {
  return [
    "You translate the command line copy of rola, a version control system for binary files.",
    `The copy belongs to the ${program} program and the translation key ${key}.`,
    `Translate from ${LANGUAGES[source]} to ${LANGUAGES[target]}.`,
    "Preserve the exact structure: every line break, every leading space, the manual column alignment, the bold, italic, bold italic, underline and strike markers, the backtick code spans, the angle bracket flags, the %{placeholder} forms, the [[directive]] forms, table pipes and separator rows, the # headings, the > quotes, the [!Warning] style alerts, the --- rules, and fenced code blocks.",
    "Translate only human-readable prose. Keep command names, flags, paths, hashes, URLs, identifiers, code and placeholders unchanged.",
    "Do not add an explanation, a preamble, a code fence, or quotes around the answer. Return only the translated text.",
  ].join("\n");
}

function cleanDetail(text) {
  return text.replace(/\s+/gu, " ").trim().slice(0, 300);
}

function dataOf(block) {
  return block
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice(5).trimStart())
    .join("\n");
}

function contentOf(choice) {
  const content = choice?.delta?.content ?? choice?.message?.content;
  if (typeof content === "string") {
    return content;
  }
  if (Array.isArray(content)) {
    return content
      .map((part) => (typeof part?.text === "string" ? part.text : ""))
      .join("");
  }
  return "";
}

/**
 * Asks the configured endpoint for a translation and writes it to `response`.
 *
 * The answer is sent as `delta`, `done` and `error` events. A failure before the
 * events begin is thrown for the server to answer as JSON; a failure after them is
 * sent as an `error` event, because the headers are already on their way.
 *
 * # Errors
 *
 * Reports a missing or malformed configuration, an endpoint that refuses the
 * request, and a request that times out. A client that goes away is not an error.
 */
export async function translateStream(
  { key, program, source, target, text },
  request,
  response,
) {
  if (typeof key !== "string" || typeof program !== "string") {
    throw new EditorError("The translation needs a key and a program");
  }
  if (
    !Object.hasOwn(LANGUAGES, source) ||
    !Object.hasOwn(LANGUAGES, target) ||
    source === target
  ) {
    throw new EditorError(
      "The translation needs two different known languages",
    );
  }
  if (typeof text !== "string" || text.trim() === "") {
    throw new EditorError("Nothing to translate");
  }
  if (text.length > 200_000) {
    throw new EditorError("The copy is too large to translate", 413);
  }

  const config = readTranslationConfig();
  const headers = {
    "Content-Type": "application/json",
    Accept: "text/event-stream",
  };
  if (config.apiKey.trim() !== "") {
    headers.Authorization = `Bearer ${config.apiKey}`;
  }
  Object.assign(headers, config.headers);

  const body = JSON.stringify({
    model: config.model,
    temperature: config.temperature,
    stream: true,
    messages: [
      {
        role: "system",
        content: promptFor({ key, program, source, target }),
      },
      { role: "user", content: text },
    ],
  });

  const controller = new AbortController();
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    controller.abort();
  }, config.timeout * 1000);
  const close = () => {
    if (!response.writableEnded) {
      controller.abort();
    }
  };
  request.on("aborted", close);
  response.on("close", close);

  const send = (event, value) => {
    if (response.destroyed || response.writableEnded) {
      return;
    }
    response.write(`event: ${event}\ndata: ${JSON.stringify(value)}\n\n`);
  };

  let started = false;
  try {
    const upstream = await fetch(endpoint(config.baseUrl), {
      method: "POST",
      headers,
      body,
      signal: controller.signal,
    });

    if (!upstream.ok) {
      const detail = await upstream.text().catch(() => "");
      throw new EditorError(
        `The endpoint answered ${upstream.status}${detail === "" ? "" : `: ${cleanDetail(detail)}`}`,
        502,
      );
    }
    if (upstream.body === null) {
      throw new EditorError("The endpoint returned no body", 502);
    }

    response.writeHead(200, {
      "Content-Type": "text/event-stream; charset=utf-8",
      "Cache-Control": "no-store",
      Connection: "keep-alive",
      "X-Accel-Buffering": "no",
      "X-Content-Type-Options": "nosniff",
    });
    started = true;

    const handle = (data) => {
      if (data === "") {
        return false;
      }
      if (data === "[DONE]") {
        return true;
      }
      let value;
      try {
        value = JSON.parse(data);
      } catch {
        return false;
      }
      if (value.error !== undefined) {
        throw new EditorError(
          value.error.message ?? "The endpoint reported an error",
          502,
        );
      }
      const content = contentOf(value.choices?.[0]);
      if (content !== "") {
        send("delta", { text: content });
      }
      return false;
    };

    const type = upstream.headers.get("content-type") ?? "";
    if (type.includes("application/json")) {
      const value = await upstream.json();
      if (value.error !== undefined) {
        throw new EditorError(
          value.error.message ?? "The endpoint reported an error",
          502,
        );
      }
      const content = contentOf(value.choices?.[0]);
      if (content !== "") {
        send("delta", { text: content });
      }
      send("done", {});
      response.end();
      return;
    }
    if (!type.includes("text/event-stream")) {
      const content = await upstream.text();
      const trimmed = content.trim();
      if (trimmed.startsWith("{")) {
        handle(content);
      } else if (/^data:/mu.test(content)) {
        for (const block of content.replace(/\r\n/gu, "\n").split("\n\n")) {
          if (handle(dataOf(block))) {
            break;
          }
        }
      } else if (content !== "") {
        send("delta", { text: content });
      }
      send("done", {});
      response.end();
      return;
    }

    const reader = upstream.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    let done = false;

    while (!done) {
      const chunk = await reader.read();
      if (chunk.done) {
        break;
      }
      buffer += decoder.decode(chunk.value, { stream: true });
      buffer = buffer.replace(/\r\n/gu, "\n");
      let at;
      while ((at = buffer.indexOf("\n\n")) >= 0) {
        const block = buffer.slice(0, at);
        buffer = buffer.slice(at + 2);
        if (handle(dataOf(block))) {
          done = true;
          break;
        }
      }
    }
    buffer += decoder.decode();
    if (!done && buffer.trim() !== "") {
      handle(dataOf(buffer));
    }

    send("done", {});
    response.end();
  } catch (error) {
    if (response.destroyed) {
      return;
    }
    if (!started) {
      if (timedOut) {
        throw new EditorError("The translation timed out", 504);
      }
      throw error;
    }
    if (!response.writableEnded) {
      send("error", {
        error: timedOut
          ? "The translation timed out"
          : (error.message ?? "The translation failed"),
      });
      response.end();
    }
  } finally {
    clearTimeout(timer);
    request.off("aborted", close);
    response.off("close", close);
  }
}
