// Reading and writing the CLI's i18n tree.
//
// The server owns the filesystem, the browser owns the editing. This module is the
// one place that turns files into keys and keys back into files, so a save and a
// reload cannot disagree about where a value sits.

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { scanCallsites } from "./callsites.mjs";
import { applyEdit, joinSource, parseFile } from "./yaml.mjs";

const PROGRAMS = [
  { name: "rola", directory: path.join("app", "cli", "rola") },
  { name: "rola-daemon", directory: path.join("app", "cli", "rola-daemon") },
  { name: "git-rola", directory: path.join("app", "cli", "git-rola") },
  { name: "rola-edit", directory: path.join("app", "cli", "rola-edit") },
];

/** The directories a CLI keeps its i18n tree under, for the server's first line. */
export const PROGRAM_DIRECTORIES = PROGRAMS.map((program) => program.directory);

const I18N = path.join(PROGRAMS[0].directory, "i18n");

/** The file the editor keeps its draft in, relative to the project root. */
export const DRAFT_FILE = "dev/cli-help-editor/__draft.yml";

/** The id the draft is listed under, so a selection can name it. */
export const DRAFT_ID = "draft:draft";

const DRAFT_LANGUAGES = new Set(["draft"]);

// A draft file the editor writes is always this shape, so the version it was
// written in is what a later reader is told.
const DRAFT_VERSION_LINE = /^version:[ \t]*(\d+)[ \t]*$/mu;

/** A failure the HTTP layer can answer with a status. */
export class EditorError extends Error {
  constructor(message, status = 400) {
    super(message);
    this.status = status;
  }
}

/** The project root, found from `start` or from this file. */
export function findRoot(start) {
  const anchor =
    start ?? path.dirname(path.dirname(fileURLToPath(import.meta.url)));
  let current = path.resolve(anchor);
  for (;;) {
    if (
      fs.existsSync(path.join(current, "Cargo.toml")) &&
      fs.existsSync(path.join(current, I18N))
    ) {
      return current;
    }
    const parent = path.dirname(current);
    if (parent === current) {
      throw new EditorError(
        `no project root above ${path.resolve(anchor)}`,
        500,
      );
    }
    current = parent;
  }
}

function relative(root, absolute) {
  return path.relative(root, absolute).split(path.sep).join("/");
}

function walk(directory) {
  const found = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (entry.name.startsWith("__")) {
      continue;
    }
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      found.push(...walk(absolute));
    } else if (entry.isFile() && /\.ya?ml$/u.test(entry.name)) {
      found.push(absolute);
    }
  }
  return found;
}

function walkRust(directory) {
  const found = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (entry.name.startsWith("__")) {
      continue;
    }
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      found.push(...walkRust(absolute));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      found.push(absolute);
    }
  }
  return found;
}

function hashOf(text) {
  return crypto.createHash("sha256").update(text, "utf8").digest("hex");
}

/** What a key draws, from its dotted path. */
export function classify(id) {
  const parts = id.split(".");
  const leaf = parts[parts.length - 1];
  const top = parts[0];
  if (top === "exit_codes") {
    return "exit-code";
  }
  if (top === "unmatched") {
    return leaf === "help" ? "hint" : "message";
  }
  if (top === "error") {
    if (leaf.startsWith("err_")) {
      return leaf.endsWith("_help") ? "hint" : "error";
    }
    return "message";
  }
  if (leaf === "help") {
    return "help";
  }
  if (leaf.endsWith("_help")) {
    return leaf.startsWith("err_") ? "hint" : "help";
  }
  if (leaf.startsWith("err_")) {
    return "error";
  }
  if (leaf === "description" || leaf.endsWith("_description")) {
    return "description";
  }
  if (parts.includes("complete")) {
    return "completion";
  }
  if (leaf === "result" || leaf.startsWith("result_")) {
    return "result";
  }
  return "message";
}

/**
 * The draft the editor always lists first.
 *
 * A draft is help copy being written, so it is kept in a file of the editor's own
 * rather than in a tree the CLI reads: a save never changes what a program prints.
 */
export function readDraft(root) {
  const absolute = path.join(root, DRAFT_FILE);
  const key = {
    id: DRAFT_ID,
    key: "draft",
    program: "draft",
    file: DRAFT_FILE,
    path: ["draft"],
    kind: "draft",
    callsites: [],
    langs: {
      draft: {
        value: "",
        style: "block",
        line: 1,
        editable: true,
        indicator: "|",
      },
    },
  };

  let text;
  try {
    text = fs.readFileSync(absolute, "utf8");
  } catch {
    return { key, problems: [] };
  }

  const parsed = parseFile(text, DRAFT_FILE, DRAFT_LANGUAGES);
  const node = parsed.nodes.find((candidate) => candidate.lang === "draft");
  if (node === undefined) {
    return { key, problems: parsed.problems };
  }

  key.langs.draft = {
    value: node.value,
    style: node.style,
    line: node.keyLine + 1,
    editable: node.editable,
    indicator: node.indicator ?? null,
  };
  return { key, problems: parsed.problems };
}

function draftVersionOf(text) {
  const found = DRAFT_VERSION_LINE.exec(text);
  return found === null ? 1 : Number(found[1]);
}

function draftSource(version, value) {
  const lines = [`version: ${String(version)}`, "", "draft: |"];
  for (const line of value.split("\n")) {
    lines.push(line === "" ? "" : `    ${line}`);
  }
  return `${lines.join("\n")}\n`;
}

// The draft is written whole: it is the editor's own file, and rewriting it keeps a
// hand-mangled one readable instead of failing on the line that was mangled.
function writeDraft(root, edits) {
  const absolute = path.join(root, DRAFT_FILE);
  const edit = edits.find(
    (candidate) => candidate.id === "draft" && candidate.lang === "draft",
  );
  if (edit === undefined || typeof edit.value !== "string") {
    throw new EditorError("the draft names no text");
  }

  let text = null;
  try {
    text = fs.readFileSync(absolute, "utf8");
  } catch {
    text = null;
  }
  if (
    text !== null &&
    typeof edit.expectedHash === "string" &&
    edit.expectedHash !== "" &&
    hashOf(text) !== edit.expectedHash
  ) {
    throw new EditorError(
      `${DRAFT_FILE} changed on disk, reload before saving`,
      409,
    );
  }

  fs.mkdirSync(path.dirname(absolute), { recursive: true });
  writeAtomic(
    absolute,
    draftSource(text === null ? 1 : draftVersionOf(text), edit.value),
  );
}

/** The keys of every CLI i18n tree, with the places each one is named. */
export function readState(root) {
  const keys = new Map();
  const fileHashes = {};
  const problems = [];

  for (const program of PROGRAMS) {
    const i18n = path.join(root, program.directory, "i18n");
    if (!fs.existsSync(i18n)) {
      continue;
    }
    const callsites = scanCallsites(
      (fs.existsSync(path.join(root, program.directory, "src"))
        ? walkRust(path.join(root, program.directory, "src"))
        : []
      ).map((absolute) => ({
        path: relative(root, absolute),
        text: fs.readFileSync(absolute, "utf8"),
      })),
    );

    for (const absolute of walk(i18n).sort()) {
      const text = fs.readFileSync(absolute, "utf8");
      const file = relative(root, absolute);
      const parsed = parseFile(text, file);
      fileHashes[file] = hashOf(text);
      problems.push(...parsed.problems);

      for (const node of parsed.nodes) {
        const id = `${program.name}:${node.id}`;
        const known = keys.get(id);
        if (known !== undefined && known.langs[node.lang] !== undefined) {
          problems.push({
            file,
            line: node.keyLine + 1,
            message: `${node.lang} is stated twice`,
          });
          continue;
        }
        if (known !== undefined && known.file !== file) {
          problems.push({
            file,
            line: node.keyLine + 1,
            message: `key is also in ${known.file}`,
          });
          continue;
        }
        const key = known ?? {
          id,
          key: node.id,
          program: program.name,
          file,
          path: node.path,
          kind: classify(node.id),
          langs: {},
          callsites: callsites.get(node.id) ?? [],
        };
        key.langs[node.lang] = {
          value: node.value,
          style: node.style,
          line: node.keyLine + 1,
          editable: node.editable,
          indicator: node.indicator ?? null,
        };
        keys.set(id, key);
      }
    }
  }

  const programOrder = new Map(
    PROGRAMS.map((program, index) => [program.name, index]),
  );
  const list = [...keys.values()].sort((left, right) => {
    const program =
      (programOrder.get(left.program) ?? Number.MAX_SAFE_INTEGER) -
      (programOrder.get(right.program) ?? Number.MAX_SAFE_INTEGER);
    if (program !== 0) {
      return program;
    }
    return left.file === right.file
      ? left.id.localeCompare(right.id)
      : left.file.localeCompare(right.file);
  });

  const draft = readDraft(root);
  problems.push(...draft.problems);
  list.unshift(draft.key);
  try {
    fileHashes[DRAFT_FILE] = hashOf(
      fs.readFileSync(path.join(root, DRAFT_FILE), "utf8"),
    );
  } catch {
    fileHashes[DRAFT_FILE] = "";
  }

  return {
    root,
    generatedAt: new Date().toISOString(),
    keys: list,
    fileHashes,
    problems,
  };
}

/** Writes a file through a temporary beside it, so a failure leaves the old one whole. */
export function writeAtomic(file, text) {
  const directory = path.dirname(file);
  const temporary = path.join(
    directory,
    `__cli-help-editor-${process.pid}-${Date.now()}.tmp`,
  );
  let mode = null;
  try {
    mode = fs.statSync(file).mode;
  } catch {
    mode = null;
  }

  fs.writeFileSync(temporary, text, "utf8");
  if (mode !== null) {
    try {
      fs.chmodSync(temporary, mode);
    } catch {
      // A filesystem without modes keeps what it has; the content is what matters.
    }
  }

  try {
    fs.renameSync(temporary, file);
  } catch (error) {
    if (error.code === "EEXIST" || error.code === "EPERM") {
      fs.unlinkSync(file);
      fs.renameSync(temporary, file);
    } else {
      try {
        fs.unlinkSync(temporary);
      } catch {
        // The temporary goes with the failure; there is nothing to report twice.
      }
      throw error;
    }
  }
}

/**
 * Writes a batch of edits, all of one file's edits at once.
 *
 * Every edit names the hash it was read from. A file that changed under the
 * editor is refused rather than overwritten, and nothing is written when one of
 * a file's edits is refused.
 */
export function writeEdits(root, edits) {
  if (!Array.isArray(edits) || edits.length === 0) {
    return readState(root);
  }

  const isDraft = (edit) =>
    edit !== null && typeof edit === "object" && edit.file === DRAFT_FILE;
  if (edits.some(isDraft)) {
    writeDraft(root, edits.filter(isDraft));
    edits = edits.filter((edit) => !isDraft(edit));
    if (edits.length === 0) {
      return readState(root);
    }
  }

  const groups = new Map();
  for (const edit of edits) {
    if (edit === null || typeof edit !== "object") {
      throw new EditorError("an edit is not an object");
    }
    const file = edit.file;
    if (
      typeof file !== "string" ||
      typeof edit.id !== "string" ||
      typeof edit.lang !== "string"
    ) {
      throw new EditorError("an edit is missing its file, key or language");
    }
    const group = groups.get(file) ?? [];
    group.push(edit);
    groups.set(file, group);
  }

  const prepared = [];

  for (const [file, group] of groups) {
    const absolute = path.resolve(root, file);
    const inside = path.relative(root, absolute);
    if (inside.startsWith("..") || path.isAbsolute(inside)) {
      throw new EditorError(`not a file in the project: ${file}`);
    }
    if (!fs.existsSync(absolute)) {
      throw new EditorError(`not a file: ${file}`, 404);
    }

    const before = fs.readFileSync(absolute, "utf8");
    const hash = hashOf(before);
    for (const edit of group) {
      if (edit.expectedHash !== hash) {
        throw new EditorError(
          `${file} changed on disk, reload before saving`,
          409,
        );
      }
    }

    const parsed = parseFile(before, file);
    const chosen = group.map((edit) => {
      const node = parsed.nodes.find(
        (candidate) => candidate.id === edit.id && candidate.lang === edit.lang,
      );
      if (node === undefined) {
        throw new EditorError(
          `${edit.id}.${edit.lang} is not a value this file holds`,
          404,
        );
      }
      if (!node.editable) {
        throw new EditorError(
          `${edit.id}.${edit.lang} is not edited by this tool`,
          422,
        );
      }
      if (typeof edit.value !== "string") {
        throw new EditorError(`${edit.id}.${edit.lang} has no text`);
      }
      return { node, value: edit.value };
    });

    chosen.sort((left, right) => right.node.keyLine - left.node.keyLine);
    prepared.push({ absolute, parsed, chosen });
  }

  // Every file is read and checked before any of them is written, so a conflict in
  // one file cannot leave another file of the same save half written.
  for (const { absolute, parsed, chosen } of prepared) {
    for (const { node, value } of chosen) {
      applyEdit(parsed.lines, node, value);
    }
    writeAtomic(absolute, joinSource(parsed));
  }

  return readState(root);
}
