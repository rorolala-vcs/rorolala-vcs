// The editor page.
//
// The server hands over the keys and their styles once. The page renders the
// source, the preview and the checks from the same modules the server checks with,
// and sends a batch of edits back when Save is pressed. Nothing in the page decides
// what a message looks like: that is `/lib/render.mjs`, a port of `cli-theme`.

import { analyzeAll, analyzeKey, summarize } from "/lib/analyze.mjs";
import { renderLines, plainOf, STANDARD, rgbOf } from "/lib/render.mjs";
import { cellsOf, displayWidth } from "/lib/width.mjs";
import { highlightText } from "/highlight.mjs";

const $ = (id) => document.getElementById(id);

const KIND = {
  help: { letter: "H", label: "Help" },
  error: { letter: "E", label: "Error" },
  hint: { letter: "h", label: "Hint" },
  result: { letter: "R", label: "Result" },
  description: { letter: "D", label: "Description" },
  completion: { letter: "C", label: "Completion" },
  "exit-code": { letter: "X", label: "Exit code" },
  draft: { letter: "D", label: "Draft" },
  message: { letter: "M", label: "Message" },
};

const LANGUAGES = [
  { id: "en", label: "EN" },
  { id: "zh-CN", label: "中文" },
];

const THEMES = [
  { id: "text", label: "Text" },
  { id: "simple", label: "Simple" },
  { id: "pretty", label: "Pretty" },
  { id: "desktop", label: "Raw" },
];

const COLOR_NAMES = [
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "white",
  "BLACK",
  "RED",
  "GREEN",
  "YELLOW",
  "BLUE",
  "MAGENTA",
  "CYAN",
  "WHITE",
];

const MAX_WIDTH = 80;
const STORAGE = "cli-help-editor";

const els = {
  banner: $("banner"),
  bannerText: $("banner-text"),
  bannerAction: $("banner-action"),
  bannerClose: $("banner-close"),
  search: $("search"),
  reload: $("reload"),
  save: $("save"),
  filters: $("filters"),
  list: $("list"),
  keyHeader: $("key-header"),
  langTabs: $("lang-tabs"),
  toolbar: $("toolbar"),
  gutter: $("gutter"),
  highlight: $("highlight"),
  editor: $("editor"),
  editorEmpty: $("editor-empty"),
  previewToolbar: $("preview-toolbar"),
  preview: $("preview"),
  previewStatus: $("preview-status"),
  checks: $("checks"),
  colorPopover: $("color-popover"),
  foregroundSwatches: $("foreground-swatches"),
  backgroundSwatches: $("background-swatches"),
  linkPopover: $("link-popover"),
  linkAddress: $("link-address"),
  linkApply: $("link-apply"),
  split: $("split"),
  toast: $("toast"),
  settings: $("settings"),
  settingsDialog: $("settings-dialog"),
  settingsPath: $("settings-path"),
  settingsBaseUrl: $("settings-base-url"),
  settingsModel: $("settings-model"),
  settingsApiKey: $("settings-api-key"),
  settingsReveal: $("settings-reveal"),
  settingsTemperature: $("settings-temperature"),
  settingsTimeout: $("settings-timeout"),
  settingsHeaders: $("settings-headers"),
  settingsHint: $("settings-hint"),
  settingsSave: $("settings-save"),
  settingsCancel: $("settings-cancel"),
};

const editorPane = document.querySelector(".editor-pane");
const previewPane = document.querySelector(".preview-pane");

function loadPrefs() {
  try {
    const raw = localStorage.getItem(STORAGE);
    return raw === null ? {} : JSON.parse(raw);
  } catch {
    return {};
  }
}

const saved = loadPrefs();

const state = {
  data: null,
  findings: [],
  summary: { total: 0, warn: 0, info: 0, kinds: {} },
  selected: saved.selected ?? null,
  lang: saved.lang === "zh-CN" ? "zh-CN" : "en",
  theme: THEMES.some((theme) => theme.id === saved.theme)
    ? saved.theme
    : "simple",
  color: saved.color !== false,
  width: Number.isFinite(saved.width) ? saved.width : 80,
  ruler: saved.ruler !== false,
  filter: "all",
  search: "",
  banner: null,
  checksOpen: false,
  checksScope: saved.checksScope === "key" ? "key" : "all",
  checksFilter: "all",
  colorMode: "fg",
  drafts: new Map(),
  pendingSelection: null,
  split: Number.isFinite(saved.split) ? saved.split : 0.5,
  settings: null,
  translation: {
    running: false,
    controller: null,
    keyId: null,
    target: null,
    text: "",
  },
};

const listItems = new Map();
let toastTimer = null;
let previewTimer = null;
let translationAnalysisTimer = null;
let cellPx = null;

function savePrefs() {
  try {
    localStorage.setItem(
      STORAGE,
      JSON.stringify({
        selected: state.selected,
        lang: state.lang,
        theme: state.theme,
        color: state.color,
        width: state.width,
        ruler: state.ruler,
        checksScope: state.checksScope,
        split: state.split,
      }),
    );
  } catch {
    // A browser that refuses storage still edits; the next visit starts fresh.
  }
}

function escapeHtml(text) {
  return String(text).replace(/[&<>"']/gu, (character) => {
    switch (character) {
      case "&":
        return "&amp;";
      case "<":
        return "&lt;";
      case ">":
        return "&gt;";
      case '"':
        return "&quot;";
      default:
        return "&#39;";
    }
  });
}

function keyById(id) {
  return state.data?.keys.find((key) => key.id === id) ?? null;
}

function selectedKey() {
  return keyById(state.selected);
}

function languageNode(key, lang = keyLang(key)) {
  return key?.langs?.[lang] ?? null;
}

/** The language a key is edited in: a draft is one text, not a pair. */
function keyLang(key, lang = state.lang) {
  return key !== null && key.kind === "draft" ? "draft" : lang;
}

/** The language a translation reads from: the one that is not being written. */
function otherLanguage(id) {
  return LANGUAGES.find((language) => language.id !== id)?.id ?? null;
}

function draftId(id, lang) {
  return `${id}\u0000${lang}`;
}

function baseline(key, lang) {
  return key?.langs?.[lang]?.value ?? null;
}

function currentValue(key = selectedKey(), lang = keyLang(key)) {
  if (!key) {
    return "";
  }
  const draft = state.drafts.get(draftId(key.id, lang));
  return draft !== undefined ? draft : (key.langs[lang]?.value ?? "");
}

function keyWithDrafts(key) {
  const langs = { ...key.langs };
  for (const language of Object.keys(key.langs)) {
    const draft = state.drafts.get(draftId(key.id, language));
    if (draft !== undefined && langs[language] !== undefined) {
      langs[language] = { ...langs[language], value: draft };
    }
  }
  return { ...key, langs };
}

function isDirty(key, lang) {
  if (!key || !key.langs[lang]) {
    return false;
  }
  const draft = state.drafts.get(draftId(key.id, lang));
  return draft !== undefined && draft !== key.langs[lang].value;
}

function anyDirty() {
  if (!state.data) {
    return false;
  }
  for (const key of state.data.keys) {
    if (Object.keys(key.langs).some((lang) => isDirty(key, lang))) {
      return true;
    }
  }
  return false;
}

function findingsFor(id) {
  return state.findings.filter((found) => found.key === id);
}

function sortFindings(list) {
  const order = { warn: 0, info: 1 };
  list.sort((left, right) => {
    const severity = (order[left.severity] ?? 2) - (order[right.severity] ?? 2);
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
  return list;
}

function previewOptions() {
  return {
    theme: state.theme,
    color: state.color,
    terminal: state.theme === "pretty" ? state.width : null,
  };
}

function applyState(data) {
  state.data = data;
  state.drafts.clear();
  state.findings = sortFindings(analyzeAll(data.keys, { maxWidth: MAX_WIDTH }));
  state.summary = summarize(state.findings);

  if (!data.keys.some((key) => key.id === state.selected)) {
    const first = data.keys.find((key) => key.kind === "help") ?? data.keys[0];
    state.selected = first?.id ?? null;
  }

  const key = selectedKey();
  if (key && !key.langs[state.lang]) {
    state.lang = key.langs.en ? "en" : key.langs["zh-CN"] ? "zh-CN" : "en";
  }

  renderAll();
  savePrefs();

  if (data.problems.length > 0) {
    toast(`${data.problems.length} file problems`, true);
    console.warn(data.problems);
  }
}

function renderAll() {
  renderBanner();
  renderFilters();
  renderList();
  renderHeader();
  renderTabs();
  renderToolbar();
  renderEditor();
  renderPreviewToolbar();
  renderPreview();
  renderChecks();
  updateSaveButton();
}

function renderBanner() {
  if (state.banner === null) {
    els.banner.hidden = true;
    return;
  }
  els.banner.hidden = false;
  els.bannerText.textContent = state.banner;
}

function showBanner(message) {
  state.banner = message;
  renderBanner();
}

function hideBanner() {
  state.banner = null;
  renderBanner();
}

function renderFilters() {
  const keys = state.data?.keys ?? [];
  const counted = keys.filter((key) => key.kind !== "draft");
  const counts = {
    all: counted.length,
    help: 0,
    error: 0,
    hint: 0,
    result: 0,
    other: 0,
  };
  for (const key of counted) {
    if (key.kind === "help") {
      counts.help += 1;
    } else if (key.kind === "error") {
      counts.error += 1;
    } else if (key.kind === "hint") {
      counts.hint += 1;
    } else if (key.kind === "result") {
      counts.result += 1;
    } else {
      counts.other += 1;
    }
  }

  const chips = [
    ["all", "All", counts.all],
    ["help", "Help", counts.help],
    ["error", "Errors", counts.error],
    ["hint", "Hints", counts.hint],
    ["result", "Results", counts.result],
    ["other", "Other", counts.other],
  ];

  els.filters.innerHTML = chips
    .map(
      ([id, label, count]) =>
        `<button class="chip" type="button" data-filter="${id}" aria-pressed="${state.filter === id}">${label}<span class="count">${count}</span></button>`,
    )
    .join("");
}

function filteredKeys() {
  const keys = state.data?.keys ?? [];
  const terms = state.search.trim().toLowerCase().split(/\s+/u).filter(Boolean);

  const draft = keys.find((key) => key.kind === "draft") ?? null;
  const rest = keys.filter((key) => {
    // The draft is the place a message is written before it belongs to a command,
    // so no filter or search takes it away.
    if (key.kind === "draft") {
      return false;
    }
    if (state.filter === "other") {
      if (["help", "error", "hint", "result"].includes(key.kind)) {
        return false;
      }
    } else if (state.filter !== "all" && key.kind !== state.filter) {
      return false;
    }
    if (terms.length === 0) {
      return true;
    }
    const haystack = [
      key.id,
      key.file,
      ...Object.values(key.langs).map((node) => node.value),
      ...key.callsites.map((site) => `${site.file} ${site.function ?? ""}`),
    ]
      .join("\n")
      .toLowerCase();
    return terms.every((term) => haystack.includes(term));
  });

  return draft === null ? rest : [draft, ...rest];
}

function dotsFor(id) {
  const findings = findingsFor(id);
  const warn = findings.some((found) => found.severity === "warn");
  const info = findings.some((found) => found.severity === "info");
  const key = keyById(id);
  const dirty =
    key !== null && Object.keys(key.langs).some((lang) => isDirty(key, lang));
  return [
    warn ? '<span class="dot warn"></span>' : "",
    info ? '<span class="dot info"></span>' : "",
    dirty ? '<span class="dot dirty"></span>' : "",
  ].join("");
}

function renderList() {
  const keys = filteredKeys();
  listItems.clear();

  if (keys.length === 0) {
    els.list.innerHTML = '<div class="empty-state">No keys match</div>';
    return;
  }

  const groups = new Map();
  for (const key of keys) {
    const group = groups.get(key.file) ?? [];
    group.push(key);
    groups.set(key.file, group);
  }

  const html = [];
  for (const [file, group] of groups) {
    html.push(`<div class="file-head">${escapeHtml(file)}</div>`);
    for (const key of group) {
      const parts = key.key.split(".");
      const leaf = parts.pop() ?? key.key;
      const parent = parts.join(".");
      const kind = KIND[key.kind] ?? KIND.message;
      html.push(
        `<button class="key-item" type="button" data-id="${escapeHtml(key.id)}" aria-current="${key.id === state.selected ? "true" : "false"}">` +
          `<span class="kind kind-${key.kind}">${kind.letter}</span>` +
          `<span class="key-name">${parent === "" ? "" : `<span class="key-parent">${escapeHtml(parent)}</span>`}<span class="key-leaf">${escapeHtml(leaf)}</span></span>` +
          `<span class="key-dots">${dotsFor(key.id)}</span>` +
          "</button>",
      );
    }
  }

  els.list.innerHTML = html.join("");
  for (const item of els.list.querySelectorAll(".key-item")) {
    listItems.set(item.dataset.id, item);
  }
  listItems.get(state.selected)?.scrollIntoView({ block: "nearest" });
}

function updateListMark(id) {
  const item = listItems.get(id);
  if (item) {
    const dots = item.querySelector(".key-dots");
    if (dots) {
      dots.innerHTML = dotsFor(id);
    }
  }
  updateSaveButton();
}

function renderHeader() {
  const key = selectedKey();
  if (!key) {
    els.keyHeader.innerHTML = '<div class="empty-state">Select a key</div>';
    return;
  }

  const kind = KIND[key.kind] ?? KIND.message;
  const missing =
    key.kind === "draft"
      ? []
      : LANGUAGES.filter((language) => !key.langs[language.id]).map(
          (language) => language.label,
        );
  const site = key.callsites[0];
  const siteText =
    site === undefined
      ? ""
      : site.function
        ? `${site.function} ${site.line}`
        : `${site.file.split("/").pop()}:${site.line}`;

  els.keyHeader.innerHTML =
    `<span class="kind kind-${key.kind}">${kind.letter}</span>` +
    `<div class="key-title"><span class="id">${escapeHtml(key.key)}</span></div>` +
    `<div class="key-meta">` +
    `<span class="badge">${escapeHtml(key.program)}</span>` +
    `<span class="path">${escapeHtml(key.file)}</span>` +
    `<span class="badge">${kind.label}</span>` +
    (site === undefined
      ? ""
      : `<span class="badge" title="${escapeHtml(`${site.file}:${site.line}`)}">${escapeHtml(siteText)}</span>`) +
    (missing.length === 0
      ? ""
      : `<span class="badge bad">${missing.join(", ")} missing</span>`) +
    "</div>";
}

function renderTabs() {
  const key = selectedKey();
  if (key !== null && key.kind === "draft") {
    els.langTabs.innerHTML =
      '<button class="tab" type="button" role="tab" aria-selected="true" disabled>Draft</button>';
    return;
  }

  const target = state.lang;
  const source = otherLanguage(target);
  const targetNode = key?.langs[target];
  const targetMissing = targetNode === undefined;
  const targetLocked = targetNode !== undefined && !targetNode.editable;
  const sourceMissing = key === null || key.langs[source] === undefined;
  const sourceEmpty = sourceMissing || currentValue(key, source).trim() === "";
  const running = state.translation.running;
  const disabled = running
    ? ""
    : targetMissing || targetLocked || sourceEmpty
      ? " disabled"
      : "";
  const title = running
    ? "Stop translation"
    : targetMissing
      ? "This language has no line in the file"
      : targetLocked
        ? "This language is not editable"
        : sourceMissing
          ? "The other language is missing"
          : sourceEmpty
            ? "The other language has nothing to translate"
            : `Translate to ${target}`;

  els.langTabs.innerHTML =
    LANGUAGES.map((language) => {
      const node = key?.langs[language.id];
      const dirty = isDirty(key, language.id);
      return (
        `<button class="tab" type="button" role="tab" data-lang="${language.id}" data-missing="${node === undefined}" aria-selected="${state.lang === language.id}">` +
        `<span>${language.label}</span>` +
        (dirty ? '<span class="dot dirty"></span>' : "") +
        "</button>"
      );
    }).join("") +
    '<span class="tab-spacer"></span>' +
    `<button id="translate" class="tab-action${running ? " stop" : ""}" type="button" title="${escapeHtml(title)}"${disabled}>${running ? "Stop" : `Translate to ${target}`}</button>`;
}

function renderToolbar() {
  const enabled =
    languageNode(selectedKey())?.editable === true &&
    !state.translation.running;
  const disabled = enabled ? "" : " disabled";

  els.toolbar.innerHTML =
    `<button class="tool bold" type="button" data-action="bold" title="Bold (Ctrl+B)"${disabled}>B</button>` +
    `<button class="tool italic" type="button" data-action="italic" title="Italic (Ctrl+I)"${disabled}>I</button>` +
    `<button class="tool mono" type="button" data-action="bolditalic" title="Bold italic"${disabled}>BI</button>` +
    `<button class="tool underline" type="button" data-action="underline" title="Underline (Ctrl+U)"${disabled}>U</button>` +
    `<button class="tool strike" type="button" data-action="strike" title="Strike"${disabled}>S</button>` +
    '<span class="separator"></span>' +
    `<button class="tool mono" type="button" data-action="code" title="Code (Ctrl+E)"${disabled}>\`</button>` +
    `<button class="tool mono" type="button" data-action="flag" title="Flag"${disabled}>&lt;&gt;</button>` +
    `<button class="tool mono" type="button" data-action="link" title="Link"${disabled}>URL</button>` +
    '<span class="separator"></span>' +
    `<button class="tool" type="button" data-action="text-color" title="Text color"${disabled}><span class="swatch-dot"></span></button>` +
    `<button class="tool" type="button" data-action="background-color" title="Background color"${disabled}><span class="swatch-dot background"></span></button>` +
    '<span class="separator"></span>' +
    `<select id="block-menu" class="block-select" title="Block"${disabled}>` +
    '<option value="">Block</option>' +
    '<option value="heading1">Heading 1</option>' +
    '<option value="heading2">Heading 2</option>' +
    '<option value="heading3">Heading 3</option>' +
    '<option value="quote">Quote</option>' +
    '<option value="warning">Warning</option>' +
    '<option value="note">Note</option>' +
    '<option value="tip">Tip</option>' +
    '<option value="caution">Caution</option>' +
    '<option value="important">Important</option>' +
    '<option value="rule">Rule</option>' +
    '<option value="table">Table</option>' +
    '<option value="fence">Code block</option>' +
    "</select>" +
    `<button class="tool mono" type="button" data-action="rule" title="Rule"${disabled}>HR</button>` +
    `<button class="tool mono" type="button" data-action="table" title="Table"${disabled}>TBL</button>` +
    `<button class="tool mono" type="button" data-action="fence" title="Code block"${disabled}>CODE</button>`;
}

function renderEditor() {
  const key = selectedKey();
  const node = languageNode(key);
  const locked = state.translation.running;
  const editable = node !== null && node.editable;
  const value =
    node !== null && (editable || locked)
      ? currentValue(key, keyLang(key))
      : "";

  els.editor.scrollTop = 0;
  els.editor.scrollLeft = 0;
  els.editor.value = value;
  els.editor.disabled = !editable || locked;
  els.editorEmpty.hidden = editable || locked;
  els.editorEmpty.textContent = node === null ? "Missing" : "Not editable";

  updateHighlight();
  updateGutter();
  syncEditorScroll();
  applyPendingSelection();
}

function renderPreviewToolbar() {
  const colorEnabled = state.theme !== "text" && state.theme !== "desktop";
  const widthEnabled = state.theme === "pretty";

  els.previewToolbar.innerHTML =
    `<div class="segment" role="group" aria-label="Theme">` +
    THEMES.map(
      (theme) =>
        `<button type="button" data-theme="${theme.id}" aria-pressed="${state.theme === theme.id}">${theme.label}</button>`,
    ).join("") +
    "</div>" +
    `<button id="color-toggle" class="toggle" type="button" aria-pressed="${state.color}"${colorEnabled ? "" : " disabled"} title="Color">Color</button>` +
    `<label class="field" title="Terminal width for Pretty rules">W <input id="preview-width" type="number" min="20" max="400" value="${state.width}"${widthEnabled ? "" : " disabled"}></label>` +
    `<button id="ruler-toggle" class="toggle" type="button" aria-pressed="${state.ruler}" title="80 column ruler">80</button>` +
    `<button id="copy-preview" class="toggle" type="button" title="Copy the preview text">Copy</button>`;
}

function cssColor(colorValue, theme) {
  const rgb = rgbOf(colorValue, theme);
  return rgb === null ? "inherit" : `rgb(${rgb[0]}, ${rgb[1]}, ${rgb[2]})`;
}

function ensureCellPx() {
  if (cellPx !== null) {
    return;
  }
  const probe = document.createElement("span");
  probe.textContent = "0000000000";
  probe.style.cssText =
    "position:fixed;visibility:hidden;white-space:pre;letter-spacing:0;" +
    "font-family:var(--preview-mono);font-size:13px;font-weight:400;font-style:normal;";
  document.body.append(probe);
  cellPx = probe.getBoundingClientRect().width / 10;
  probe.remove();
  document.documentElement.style.setProperty("--cell-px", `${cellPx}px`);
}

function styleDeclaration(style, theme) {
  const parts = [];
  let foreground = style.fg === null ? null : cssColor(style.fg, theme);
  let background = style.bg === null ? null : cssColor(style.bg, theme);

  if (style.reverse) {
    const oldForeground = foreground;
    foreground = background ?? "var(--term-bg)";
    background = oldForeground ?? "var(--term-fg)";
  }

  if (foreground !== null) {
    parts.push(`color:${foreground}`);
  }
  if (background !== null) {
    parts.push(`background-color:${background}`);
  }
  if (style.bold) {
    parts.push("font-weight:700");
  }
  if (style.italic) {
    parts.push("font-style:italic");
  }
  if (style.underline) {
    parts.push("text-decoration:underline");
  }
  if (style.strike) {
    parts.push("text-decoration:line-through");
  }
  return parts.join(";");
}

function runsToHtml(runs, theme) {
  return runs
    .map((run) => {
      const declaration = styleDeclaration(run.style, theme);
      const groups = cellsOf(run.text)
        .map((group) => {
          const style = [
            group.cells > 0
              ? `width:calc(${group.cells} * var(--cell-px))`
              : "",
            declaration,
          ]
            .filter(Boolean)
            .join(";");
          return `<span class="cell"${style === "" ? "" : ` style="${style}"`}>${escapeHtml(group.text)}</span>`;
        })
        .join("");
      if (run.style.link === null) {
        return groups;
      }
      const scheme = /^([a-zA-Z][a-zA-Z0-9+.-]*):/u.exec(run.style.link);
      const safe =
        scheme === null ||
        ["http", "https", "mailto"].includes(scheme[1].toLowerCase());
      return safe
        ? `<a href="${escapeHtml(run.style.link)}" target="_blank" rel="noreferrer">${groups}</a>`
        : groups;
    })
    .join("");
}

function renderPreview() {
  const key = selectedKey();
  const node = languageNode(key);
  const previousTop = els.preview.scrollTop;
  const previousLeft = els.preview.scrollLeft;

  if (key === null || node === null) {
    els.preview.innerHTML =
      '<div class="preview-inner"><div class="preview-line"><span class="preview-number">1</span><span class="preview-content"></span></div></div>';
    els.previewStatus.innerHTML = "<span>Missing</span>";
    return;
  }

  const rendered = renderLines(
    currentValue(key, keyLang(key)),
    previewOptions(),
  );
  const lines = rendered.lines.length === 0 ? [[]] : rendered.lines;
  const widths = lines.map((runs) => displayWidth(plainOf(runs)));
  const widest = widths.length === 0 ? 0 : Math.max(...widths);
  const over = widths.filter((width) => width > MAX_WIDTH).length;

  const body = lines
    .map(
      (runs, index) =>
        `<div class="preview-line"><span class="preview-number">${index + 1}</span><span class="preview-content">${runsToHtml(runs, state.theme)}</span></div>`,
    )
    .join("");

  els.preview.innerHTML = `<div class="preview-inner">${state.ruler ? '<div class="ruler"></div>' : ""}${body}</div>`;
  els.preview.scrollTop = previousTop;
  els.preview.scrollLeft = previousLeft;

  els.previewStatus.innerHTML =
    `<span>${lines.length} lines</span>` +
    `<span class="${widest > MAX_WIDTH ? "over" : ""}">max ${widest}</span>` +
    (over === 0 ? "" : `<span class="over">${over} over ${MAX_WIDTH}</span>`);
}

function schedulePreview() {
  clearTimeout(previewTimer);
  previewTimer = setTimeout(renderPreview, 40);
}

function renderChecks() {
  const key = selectedKey();
  const scoped =
    state.checksScope === "key"
      ? key === null
        ? []
        : findingsFor(key.id)
      : state.findings;
  const filtered = scoped.filter(
    (found) =>
      state.checksFilter === "all" || found.kind === state.checksFilter,
  );
  const warn = scoped.filter((found) => found.severity === "warn").length;
  const info = scoped.filter((found) => found.severity === "info").length;
  const shown = filtered.slice(0, 300);

  const counts = [
    warn > 0 ? `<span>${warn} warn</span>` : "",
    info > 0 ? `<span>${info} info</span>` : "",
    warn === 0 && info === 0 ? "<span>clean</span>" : "",
  ].join("");

  const filters = [
    ["all", "All"],
    ["alignment", "Alignment"],
    ["overlong", "Overlong"],
    ["copy", "Copy"],
    ["placeholder", "Placeholder"],
    ["missing", "Missing"],
    ["markup", "Markup"],
    ["empty", "Empty"],
  ];

  const findings = shown
    .map((found) => {
      const language =
        found.lang === "zh-CN" ? "ZH" : found.lang === "draft" ? "DRAFT" : "EN";
      return (
        `<button class="finding" type="button" data-key="${escapeHtml(found.key)}" data-lang="${found.lang}" data-line="${found.line ?? 0}" data-offset="${found.offset ?? ""}" data-length="${found.length ?? 0}">` +
        `<span class="sev ${found.severity}"></span>` +
        `<span class="finding-key">${escapeHtml(found.key)}</span>` +
        `<span class="finding-lang">${language}</span>` +
        `<span class="finding-message">${escapeHtml(found.message)}</span>` +
        `<span class="finding-text">${escapeHtml(found.text ?? "")}</span>` +
        "</button>"
      );
    })
    .join("");

  const empty =
    filtered.length === 0 ? '<div class="empty-state">No findings</div>' : "";
  const more =
    filtered.length > shown.length
      ? `<div class="empty-state">Showing ${shown.length} of ${filtered.length}</div>`
      : "";

  els.checks.classList.toggle("open", state.checksOpen);
  els.checks.innerHTML =
    '<div class="checks-head">' +
    `<button class="checks-toggle" id="checks-toggle" type="button" aria-expanded="${state.checksOpen}"><span class="title">Checks</span><span class="counts">${counts}</span></button>` +
    '<span class="spacer"></span>' +
    `<select id="checks-filter" class="block-select" title="Check">${filters
      .map(
        ([id, label]) =>
          `<option value="${id}"${state.checksFilter === id ? " selected" : ""}>${label}</option>`,
      )
      .join("")}</select>` +
    '<span class="checks-scope">' +
    `<button type="button" data-scope="key" aria-pressed="${state.checksScope === "key"}">Key</button>` +
    `<button type="button" data-scope="all" aria-pressed="${state.checksScope === "all"}">All</button>` +
    "</span>" +
    "</div>" +
    `<div class="checks-list">${findings}${empty}${more}</div>`;
}

function updateSaveButton() {
  const running = state.translation.running;
  const dirty = anyDirty();
  els.save.disabled = running || !dirty;
  els.save.dataset.dirty = String(dirty && !running);
  els.reload.disabled = running;
}

function updateHighlight() {
  els.highlight.innerHTML = `<pre>${highlightText(els.editor.value, "simple")}</pre>`;
  syncEditorScroll();
}

function updateGutter() {
  const count = els.editor.value.split("\n").length;
  const numbers = [];
  for (let index = 1; index <= count; index += 1) {
    numbers.push(`<div>${index}</div>`);
  }
  els.gutter.innerHTML = `<div class="gutter-inner">${numbers.join("")}</div>`;
  syncEditorScroll();
}

function syncEditorScroll() {
  const pre = els.highlight.querySelector("pre");
  if (pre !== null) {
    pre.style.transform = `translate(${-els.editor.scrollLeft}px, ${-els.editor.scrollTop}px)`;
  }
  const inner = els.gutter.querySelector(".gutter-inner");
  if (inner !== null) {
    inner.style.transform = `translateY(${-els.editor.scrollTop}px)`;
  }
}

function refreshFindingsForKey(id) {
  const key = keyById(id);
  if (key === null) {
    return;
  }
  state.findings = sortFindings(
    state.findings
      .filter((found) => found.key !== id)
      .concat(analyzeKey(keyWithDrafts(key), { maxWidth: MAX_WIDTH })),
  );
  state.summary = summarize(state.findings);
}

function onInput() {
  if (state.translation.running) {
    return;
  }
  const key = selectedKey();
  if (key === null || !key.langs[keyLang(key)]) {
    return;
  }
  const value = els.editor.value;
  const id = draftId(key.id, keyLang(key));
  if (value === key.langs[keyLang(key)].value) {
    state.drafts.delete(id);
  } else {
    state.drafts.set(id, value);
  }

  updateHighlight();
  updateGutter();
  renderTabs();
  updateListMark(key.id);
  refreshFindingsForKey(key.id);
  schedulePreview();
  renderChecks();
  updateSaveButton();
}

function replaceRange(start, end, text, selectionStart, selectionEnd) {
  els.editor.focus();
  els.editor.setRangeText(text, start, end, "end");
  if (selectionStart !== undefined) {
    els.editor.setSelectionRange(selectionStart, selectionEnd);
  }
  onInput();
}

function wrap(open, close) {
  const value = els.editor.value;
  const start = els.editor.selectionStart;
  const end = els.editor.selectionEnd;

  if (start === end) {
    const text = open + close;
    replaceRange(start, end, text, start + open.length, start + open.length);
    return;
  }

  const selected = value.slice(start, end);
  if (
    selected.startsWith(open) &&
    selected.endsWith(close) &&
    selected.length >= open.length + close.length
  ) {
    const inner = selected.slice(open.length, selected.length - close.length);
    replaceRange(start, end, inner, start, start + inner.length);
    return;
  }

  if (
    value.slice(Math.max(0, start - open.length), start) === open &&
    value.slice(end, end + close.length) === close
  ) {
    const from = start - open.length;
    replaceRange(
      from,
      end + close.length,
      selected,
      from,
      from + selected.length,
    );
    return;
  }

  const text = open + selected + close;
  replaceRange(
    start,
    end,
    text,
    start + open.length,
    start + open.length + selected.length,
  );
}

function selectedLines() {
  const value = els.editor.value;
  const start = els.editor.selectionStart;
  const end = els.editor.selectionEnd;
  const from = value.lastIndexOf("\n", start - 1) + 1;
  let to = value.indexOf("\n", end);
  if (to < 0) {
    to = value.length;
  }
  return { value, start, end, from, to, block: value.slice(from, to) };
}

function prefixLines(prefix) {
  const lines = selectedLines();
  let block = lines.block.split("\n");
  if (prefix.startsWith("#")) {
    block = block.map((line) => line.replace(/^#{1,6} /u, ""));
  }
  const filled = block.filter((line) => line.trim() !== "");
  const present =
    filled.length > 0 && filled.every((line) => line.startsWith(prefix));
  const next = block
    .map((line) => {
      if (line.trim() === "") {
        return line;
      }
      return present ? line.slice(prefix.length) : prefix + line;
    })
    .join("\n");
  replaceRange(
    lines.from,
    lines.to,
    next,
    lines.from + next.length,
    lines.from + next.length,
  );
}

function insertAlert(kind) {
  const lines = selectedLines();
  if (lines.block.trim() === "") {
    const text = `> [!${kind}]\n> `;
    replaceRange(
      lines.from,
      lines.to,
      text,
      lines.from + text.length,
      lines.from + text.length,
    );
    return;
  }
  const body = lines.block
    .split("\n")
    .map((line) => (line.trim() === "" ? ">" : `> ${line}`))
    .join("\n");
  const text = `> [!${kind}]\n${body}`;
  replaceRange(
    lines.from,
    lines.to,
    text,
    lines.from + text.length,
    lines.from + text.length,
  );
}

function insertRule() {
  const value = els.editor.value;
  const start = els.editor.selectionStart;
  const lineStart = value.lastIndexOf("\n", start - 1) + 1;
  const nextBreak = value.indexOf("\n", start);
  const lineEnd = nextBreak < 0 ? value.length : nextBreak;
  const rest = value.slice(lineStart, lineEnd);
  if (rest.trim() === "") {
    replaceRange(lineStart, lineEnd, "---\n", lineStart + 4, lineStart + 4);
    return;
  }
  replaceRange(lineEnd, lineEnd, "\n---\n", lineEnd + 5, lineEnd + 5);
}

function insertTable() {
  const template = "| Name | Value |\n| --- | --- |\n|  |  |";
  const start = els.editor.selectionStart;
  const nameAt = start + 2;
  replaceRange(start, els.editor.selectionEnd, template, nameAt, nameAt + 4);
}

function insertFence() {
  const fence = "```";
  const start = els.editor.selectionStart;
  const end = els.editor.selectionEnd;
  const selected = els.editor.value.slice(start, end);
  if (selected === "") {
    const text = `${fence}\n\n${fence}`;
    replaceRange(
      start,
      end,
      text,
      start + fence.length + 1,
      start + fence.length + 1,
    );
  } else {
    const text = `${fence}\n${selected}\n${fence}`;
    replaceRange(
      start,
      end,
      text,
      start + fence.length + 1,
      start + fence.length + 1 + selected.length,
    );
  }
}

function applyBlock(value) {
  switch (value) {
    case "heading1":
      prefixLines("# ");
      break;
    case "heading2":
      prefixLines("## ");
      break;
    case "heading3":
      prefixLines("### ");
      break;
    case "quote":
      prefixLines("> ");
      break;
    case "warning":
      insertAlert("Warning");
      break;
    case "note":
      insertAlert("Note");
      break;
    case "tip":
      insertAlert("Tip");
      break;
    case "caution":
      insertAlert("Caution");
      break;
    case "important":
      insertAlert("Important");
      break;
    case "rule":
      insertRule();
      break;
    case "table":
      insertTable();
      break;
    case "fence":
      insertFence();
      break;
    default:
      break;
  }
}

function applyAction(action, button) {
  switch (action) {
    case "bold":
      wrap("**", "**");
      break;
    case "italic":
      wrap("*", "*");
      break;
    case "bolditalic":
      wrap("***", "***");
      break;
    case "underline":
      wrap("__", "__");
      break;
    case "strike":
      wrap("~~", "~~");
      break;
    case "code":
      wrap("`", "`");
      break;
    case "flag":
      wrap("<", ">");
      break;
    case "link":
      openLinkPopover(button);
      break;
    case "text-color":
      openColorPopover(button, "fg");
      break;
    case "background-color":
      openColorPopover(button, "bg");
      break;
    case "rule":
      insertRule();
      break;
    case "table":
      insertTable();
      break;
    case "fence":
      insertFence();
      break;
    default:
      break;
  }
}

function indentSelection(outdent) {
  const lines = selectedLines();
  const block = lines.block.split("\n");
  if (lines.start === lines.end && !outdent) {
    replaceRange(
      lines.start,
      lines.end,
      "    ",
      lines.start + 4,
      lines.start + 4,
    );
    return;
  }
  const next = block
    .map((line) => {
      if (outdent) {
        return line.replace(/^ {1,4}/u, "");
      }
      return `    ${line}`;
    })
    .join("\n");
  replaceRange(
    lines.from,
    lines.to,
    next,
    lines.from,
    lines.from + next.length,
  );
}

function offsetOfLine(value, line) {
  let at = 0;
  for (let index = 0; index < line; index += 1) {
    const found = value.indexOf("\n", at);
    if (found < 0) {
      return value.length;
    }
    at = found + 1;
  }
  return at;
}

function focusSelection(start, end) {
  els.editor.focus();
  els.editor.setSelectionRange(start, end);
  const line = els.editor.value.slice(0, start).split("\n").length - 1;
  const height =
    Number.parseFloat(getComputedStyle(els.editor).lineHeight) || 20;
  els.editor.scrollTop = Math.max(
    0,
    line * height - els.editor.clientHeight / 2,
  );
  syncEditorScroll();
}

function applyPendingSelection() {
  const pending = state.pendingSelection;
  if (pending === null) {
    return;
  }
  state.pendingSelection = null;
  const value = els.editor.value;
  let start = pending.offset;
  if (!Number.isFinite(start)) {
    const key = selectedKey();
    const node = languageNode(key);
    const from = key === null || node === null ? 0 : node.line;
    const valueLine =
      node === null
        ? 0
        : node.style === "block"
          ? Math.max(0, (pending.line ?? from + 1) - from - 1)
          : 0;
    start = offsetOfLine(value, valueLine);
  }
  const length = Number.isFinite(pending.length) ? pending.length : 0;
  focusSelection(start, start + length);
}

function selectKey(id, options = {}) {
  if (state.translation.running) {
    return;
  }
  const key = keyById(id);
  if (key === null) {
    return;
  }
  state.selected = id;
  if (options.lang !== undefined && key.langs[options.lang]) {
    state.lang = options.lang;
  } else if (!key.langs[state.lang]) {
    state.lang = key.langs.en ? "en" : key.langs["zh-CN"] ? "zh-CN" : "en";
  }

  for (const [itemId, item] of listItems) {
    item.setAttribute("aria-current", itemId === id ? "true" : "false");
  }
  listItems.get(id)?.scrollIntoView({ block: "nearest" });
  savePrefs();

  renderHeader();
  renderTabs();
  renderToolbar();
  renderEditor();
  renderPreview();
  renderChecks();
  updateSaveButton();
}

function switchLanguage(lang) {
  if (state.translation.running) {
    return;
  }
  state.lang = lang;
  savePrefs();
  renderTabs();
  renderToolbar();
  renderEditor();
  renderPreview();
  renderChecks();
  updateSaveButton();
}

function navigateToFinding(button) {
  const id = button.dataset.key;
  const lang = button.dataset.lang;
  const offset =
    button.dataset.offset === "" ? null : Number(button.dataset.offset);
  const length = Number(button.dataset.length ?? 0);
  const line = Number(button.dataset.line ?? 0);
  state.pendingSelection = { offset, length, line };
  if (state.selected === id && state.lang === lang) {
    applyPendingSelection();
    return;
  }
  selectKey(id, { lang });
}

async function saveAll() {
  if (state.translation.running || !anyDirty()) {
    return;
  }
  const edits = [];
  for (const [id, value] of state.drafts) {
    const [keyId, lang] = id.split("\u0000");
    const key = keyById(keyId);
    if (key === null || !key.langs[lang]) {
      continue;
    }
    if (value === key.langs[lang].value) {
      continue;
    }
    edits.push({
      file: key.file,
      id: key.key,
      lang,
      value,
      expectedHash: state.data.fileHashes[key.file],
    });
  }
  if (edits.length === 0) {
    state.drafts.clear();
    updateSaveButton();
    return;
  }

  els.save.disabled = true;
  try {
    const response = await fetch("/api/save", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ edits }),
    });
    const body = await response.json();
    if (!response.ok) {
      if (response.status === 409) {
        showBanner(body.error ?? "File changed on disk");
      } else {
        toast(body.error ?? "Save failed", true);
      }
      return;
    }
    hideBanner();
    applyState(body);
    toast("Saved");
  } catch (error) {
    toast(error.message ?? "Save failed", true);
  } finally {
    updateSaveButton();
  }
}

async function reload() {
  if (state.translation.running) {
    return;
  }
  if (anyDirty() && !window.confirm("Discard unsaved edits?")) {
    return;
  }
  try {
    const response = await fetch("/api/state");
    const body = await response.json();
    if (!response.ok) {
      toast(body.error ?? "Reload failed", true);
      return;
    }
    hideBanner();
    applyState(body);
    toast("Reloaded");
  } catch (error) {
    toast(error.message ?? "Reload failed", true);
  }
}

function toast(message, bad = false, duration = 2200) {
  els.toast.textContent = message;
  els.toast.className = bad ? "toast bad" : "toast";
  els.toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    els.toast.hidden = true;
  }, duration);
}

function setSettingsHint(message, bad = false) {
  els.settingsHint.textContent = message;
  els.settingsHint.title = message;
  els.settingsHint.classList.toggle("bad", bad);
}

function maskKey() {
  els.settingsApiKey.type = "password";
  els.settingsReveal.textContent = "Show";
  els.settingsReveal.setAttribute("aria-pressed", "false");
}

function fillSettings(settings) {
  state.settings = settings;
  els.settingsPath.textContent = settings.file;
  els.settingsPath.title = settings.file;
  els.settingsBaseUrl.value = settings.config.baseUrl;
  els.settingsModel.value = settings.config.model;
  els.settingsApiKey.value = settings.config.apiKey;
  els.settingsTemperature.value = String(settings.config.temperature);
  els.settingsTimeout.value = String(settings.config.timeout);
  els.settingsHeaders.value = Object.entries(settings.config.headers)
    .map(([name, value]) => `${name}: ${value}`)
    .join("\n");
  maskKey();
  setSettingsHint(settings.error ?? "", settings.error !== null);
}

async function openSettings() {
  if (!els.settingsDialog.hidden) {
    return;
  }

  let settings;
  try {
    const response = await fetch("/api/settings");
    if (!response.ok) {
      throw new Error(`The settings could not be read (${response.status})`);
    }
    settings = await response.json();
  } catch (error) {
    toast(error.message ?? "Could not read the settings", true);
    return;
  }

  fillSettings(settings);
  els.settingsDialog.hidden = false;
  els.settingsBaseUrl.focus();
  els.settingsBaseUrl.select();
}

function closeSettings() {
  els.settingsDialog.hidden = true;
  maskKey();
}

function settingsHeaders() {
  const headers = {};
  for (const line of els.settingsHeaders.value.split(/\r?\n/u)) {
    const trimmed = line.trim();
    if (trimmed === "") {
      continue;
    }
    const at = trimmed.indexOf(":");
    if (at <= 0) {
      throw new Error("headers: each line is Name: value");
    }
    headers[trimmed.slice(0, at).trim()] = trimmed.slice(at + 1).trim();
  }
  return headers;
}

async function saveSettings() {
  let headers;
  try {
    headers = settingsHeaders();
  } catch (error) {
    setSettingsHint(error.message, true);
    return;
  }

  els.settingsSave.disabled = true;
  setSettingsHint("Saving");
  try {
    const response = await fetch("/api/settings", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        baseUrl: els.settingsBaseUrl.value,
        model: els.settingsModel.value,
        apiKey: els.settingsApiKey.value,
        temperature:
          els.settingsTemperature.value === ""
            ? null
            : Number(els.settingsTemperature.value),
        timeout:
          els.settingsTimeout.value === ""
            ? null
            : Number(els.settingsTimeout.value),
        headers,
      }),
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) {
      setSettingsHint(
        body.error ?? `The settings were refused (${response.status})`,
        true,
      );
      return;
    }
    state.settings = body;
    if (body.error !== null && body.error !== undefined) {
      setSettingsHint(body.error, true);
      return;
    }
    toast("Saved");
    closeSettings();
  } catch (error) {
    setSettingsHint(error.message ?? "The settings could not be written", true);
  } finally {
    els.settingsSave.disabled = false;
  }
}

function openColorPopover(button, mode) {
  state.colorMode = mode;
  const rect = button.getBoundingClientRect();
  els.colorPopover.hidden = false;
  els.colorPopover.style.left = `${Math.max(8, Math.min(rect.left, window.innerWidth - 230))}px`;
  els.colorPopover.style.top = `${rect.bottom + 6}px`;
  els.linkPopover.hidden = true;
}

function openLinkPopover(button) {
  const rect = button.getBoundingClientRect();
  els.linkPopover.hidden = false;
  els.linkPopover.style.left = `${Math.max(8, Math.min(rect.left, window.innerWidth - 330))}px`;
  els.linkPopover.style.top = `${rect.bottom + 6}px`;
  els.colorPopover.hidden = true;
  els.linkAddress.value = "";
  els.linkAddress.focus();
}

function closePopovers() {
  els.colorPopover.hidden = true;
  els.linkPopover.hidden = true;
}

function applyColor(name) {
  const open = state.colorMode === "bg" ? `[[~${name}]]` : `[[${name}]]`;
  wrap(open, "[[/]]");
  closePopovers();
}

function applyLink() {
  const address = els.linkAddress.value.trim().replace(/\]\]/gu, "");
  if (address === "") {
    closePopovers();
    return;
  }
  wrap(`[[?${address}]]`, "[[/]]");
  closePopovers();
}

function setTheme(theme) {
  state.theme = theme;
  savePrefs();
  renderPreviewToolbar();
  renderPreview();
}

function setColor(color) {
  state.color = color;
  savePrefs();
  renderPreviewToolbar();
  renderPreview();
}

function setRuler(ruler) {
  state.ruler = ruler;
  savePrefs();
  renderPreviewToolbar();
  renderPreview();
}

function setWidth(width) {
  if (!Number.isFinite(width) || width < 20 || width > 400) {
    return;
  }
  state.width = Math.round(width);
  savePrefs();
  renderPreview();
}

async function copyPreview() {
  const key = selectedKey();
  if (key === null || languageNode(key) === null) {
    return;
  }
  const text = renderLines(currentValue(key, keyLang(key)), previewOptions())
    .lines.map(plainOf)
    .join("\n");
  try {
    await navigator.clipboard.writeText(text);
    toast("Copied");
  } catch {
    toast("Copy failed", true);
  }
}

function initSwatches() {
  const make = () =>
    COLOR_NAMES.map((name, index) => {
      const rgb = STANDARD[index];
      const label =
        name === name.toUpperCase() ? `${name.toLowerCase()} bright` : name;
      return `<button class="swatch" type="button" data-name="${name}" title="${label}" style="background:rgb(${rgb[0]}, ${rgb[1]}, ${rgb[2]})"></button>`;
    }).join("");
  els.foregroundSwatches.innerHTML = make();
  els.backgroundSwatches.innerHTML = make();
}

function parseSseBlock(block) {
  let event = "message";
  const data = [];
  for (const line of block.split("\n")) {
    if (line.startsWith("event:")) {
      event = line.slice(6).trim();
    } else if (line.startsWith("data:")) {
      data.push(line.slice(5).trimStart());
    }
  }
  if (data.length === 0) {
    return null;
  }
  const raw = data.join("\n");
  try {
    return { event, data: JSON.parse(raw) };
  } catch {
    return { event, data: raw };
  }
}

async function readSseStream(body, onEvent) {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  for (;;) {
    const chunk = await reader.read();
    if (chunk.done) {
      break;
    }
    buffer += decoder.decode(chunk.value, { stream: true });
    let at;
    while ((at = buffer.indexOf("\n\n")) >= 0) {
      const block = buffer.slice(0, at);
      buffer = buffer.slice(at + 2);
      const event = parseSseBlock(block);
      if (event !== null && onEvent(event) === false) {
        await reader.cancel();
        return;
      }
    }
  }

  buffer += decoder.decode();
  if (buffer.trim() !== "") {
    const event = parseSseBlock(buffer);
    if (event !== null) {
      onEvent(event);
    }
  }
}

function scheduleTranslationAnalysis(id) {
  if (translationAnalysisTimer !== null) {
    return;
  }
  translationAnalysisTimer = setTimeout(() => {
    translationAnalysisTimer = null;
    refreshFindingsForKey(id);
    renderChecks();
  }, 250);
}

function appendTranslation(text) {
  if (text === "") {
    return;
  }
  const { keyId, target } = state.translation;
  const key = keyById(keyId);
  if (key === null) {
    return;
  }
  const follow =
    els.editor.scrollTop + els.editor.clientHeight >=
    els.editor.scrollHeight - 24;
  state.translation.text += text;
  state.drafts.set(draftId(key.id, target), state.translation.text);
  if (state.lang === target) {
    els.editor.value = state.translation.text;
    updateHighlight();
    updateGutter();
    if (follow) {
      els.editor.scrollTop = els.editor.scrollHeight;
    }
    syncEditorScroll();
  }
  updateListMark(key.id);
  schedulePreview();
  scheduleTranslationAnalysis(key.id);
  updateSaveButton();
}

function finalizeTranslation({ error = null, aborted = false } = {}) {
  if (!state.translation.running) {
    return;
  }
  const { keyId, target, text: raw } = state.translation;
  const key = keyById(keyId);
  const produced = raw.replace(/^\n+/u, "").replace(/\s+$/u, "");

  if (key !== null && raw !== "") {
    if (produced === "") {
      state.drafts.delete(draftId(key.id, target));
    } else {
      state.drafts.set(draftId(key.id, target), produced);
    }
  }

  state.translation.running = false;
  state.translation.controller = null;
  if (translationAnalysisTimer !== null) {
    clearTimeout(translationAnalysisTimer);
    translationAnalysisTimer = null;
  }
  if (key !== null) {
    refreshFindingsForKey(key.id);
  }

  renderTabs();
  renderToolbar();
  renderEditor();
  renderPreview();
  renderChecks();
  updateSaveButton();

  if (error !== null) {
    toast(error.message ?? "Translation failed", true, 6000);
  } else if (aborted) {
    return;
  } else if (produced !== "") {
    toast("Translated");
  } else {
    toast("The endpoint returned no text", true, 6000);
  }
}

function stopTranslation() {
  state.translation.controller?.abort();
}

async function translate() {
  if (state.translation.running) {
    stopTranslation();
    return;
  }

  const key = selectedKey();
  if (key === null || key.kind === "draft") {
    return;
  }
  const target = state.lang;
  const source = otherLanguage(target);
  if (source === null) {
    toast("There is no other language", true);
    return;
  }
  if (key.langs[target] === undefined) {
    toast("This language has no line in the file", true);
    return;
  }
  if (!key.langs[target].editable) {
    toast("This language is not editable", true);
    return;
  }
  if (key.langs[source] === undefined) {
    toast("The other language is missing", true);
    return;
  }
  const text = currentValue(key, source);
  if (text.trim() === "") {
    toast("The other language has nothing to translate", true);
    return;
  }

  state.translation = {
    running: true,
    controller: new AbortController(),
    keyId: key.id,
    target,
    text: "",
  };
  renderTabs();
  renderToolbar();
  renderEditor();
  renderPreview();
  renderChecks();
  updateSaveButton();

  try {
    const response = await fetch("/api/translate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        program: key.program,
        key: key.key,
        source,
        target,
        text,
      }),
      signal: state.translation.controller.signal,
    });

    if (!response.ok) {
      const body = await response.json().catch(() => ({}));
      const failure = new Error(
        body.error ?? `The translation failed with status ${response.status}`,
      );
      failure.status = response.status;
      throw failure;
    }
    if (response.body === null) {
      throw new Error("The translation returned no body");
    }

    await readSseStream(response.body, (event) => {
      if (event.event === "delta") {
        if (event.data !== null && typeof event.data.text === "string") {
          appendTranslation(event.data.text);
        }
        return true;
      }
      if (event.event === "error") {
        throw new Error(
          typeof event.data === "string"
            ? event.data
            : (event.data?.error ?? "Translation failed"),
        );
      }
      return event.event !== "done";
    });
    finalizeTranslation({});
  } catch (error) {
    state.translation.controller?.abort();
    finalizeTranslation({
      error: error.name === "AbortError" ? null : error,
      aborted: error.name === "AbortError",
    });
    if (error.status === 503) {
      openSettings();
    }
  }
}

function bindEvents() {
  els.editor.addEventListener("input", onInput);
  els.editor.addEventListener("scroll", syncEditorScroll);
  els.editor.addEventListener("keydown", (event) => {
    if (event.key === "Tab") {
      event.preventDefault();
      indentSelection(event.shiftKey);
    }
  });

  els.list.addEventListener("click", (event) => {
    const item = event.target.closest(".key-item");
    if (item !== null) {
      selectKey(item.dataset.id);
    }
  });

  els.filters.addEventListener("click", (event) => {
    const chip = event.target.closest(".chip");
    if (chip !== null) {
      state.filter = chip.dataset.filter;
      renderFilters();
      renderList();
    }
  });

  els.langTabs.addEventListener("click", (event) => {
    if (event.target.closest("#translate") !== null) {
      translate();
      return;
    }
    const tab = event.target.closest(".tab");
    if (tab !== null) {
      switchLanguage(tab.dataset.lang);
    }
  });

  els.toolbar.addEventListener("click", (event) => {
    const button = event.target.closest("[data-action]");
    if (button !== null && !button.disabled) {
      applyAction(button.dataset.action, button);
    }
  });
  els.toolbar.addEventListener("change", (event) => {
    if (event.target.id === "block-menu") {
      const value = event.target.value;
      event.target.value = "";
      if (value !== "") {
        applyBlock(value);
      }
    }
  });

  els.previewToolbar.addEventListener("click", (event) => {
    const theme = event.target.closest("[data-theme]");
    if (theme !== null) {
      setTheme(theme.dataset.theme);
      return;
    }
    if (event.target.id === "color-toggle") {
      setColor(!state.color);
      return;
    }
    if (event.target.id === "ruler-toggle") {
      setRuler(!state.ruler);
      return;
    }
    if (event.target.id === "copy-preview") {
      copyPreview();
    }
  });
  els.previewToolbar.addEventListener("input", (event) => {
    if (event.target.id === "preview-width") {
      setWidth(Number(event.target.value));
    }
  });

  els.checks.addEventListener("click", (event) => {
    if (event.target.closest("#checks-toggle") !== null) {
      state.checksOpen = !state.checksOpen;
      renderChecks();
      return;
    }
    const scope = event.target.closest("[data-scope]");
    if (scope !== null) {
      state.checksScope = scope.dataset.scope;
      savePrefs();
      renderChecks();
      return;
    }
    const finding = event.target.closest(".finding");
    if (finding !== null) {
      navigateToFinding(finding);
    }
  });
  els.checks.addEventListener("change", (event) => {
    if (event.target.id === "checks-filter") {
      state.checksFilter = event.target.value;
      renderChecks();
    }
  });

  els.search.addEventListener("input", () => {
    state.search = els.search.value;
    renderList();
  });
  els.search.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      const first = filteredKeys()[0];
      if (first !== undefined) {
        selectKey(first.id);
      }
    }
  });

  els.save.addEventListener("click", saveAll);
  els.reload.addEventListener("click", reload);
  els.bannerAction.addEventListener("click", reload);
  els.bannerClose.addEventListener("click", hideBanner);

  els.settings.addEventListener("click", openSettings);
  els.settingsCancel.addEventListener("click", closeSettings);
  els.settingsSave.addEventListener("click", saveSettings);
  els.settingsReveal.addEventListener("click", () => {
    const revealing = els.settingsApiKey.type === "password";
    els.settingsApiKey.type = revealing ? "text" : "password";
    els.settingsReveal.textContent = revealing ? "Hide" : "Show";
    els.settingsReveal.setAttribute("aria-pressed", String(revealing));
    els.settingsApiKey.focus();
  });
  els.settingsDialog.addEventListener("mousedown", (event) => {
    if (event.target === els.settingsDialog) {
      closeSettings();
    }
  });
  els.settingsDialog.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" || event.target instanceof HTMLTextAreaElement) {
      return;
    }
    if (
      event.target instanceof HTMLElement &&
      event.target.closest("button") !== null
    ) {
      return;
    }
    event.preventDefault();
    saveSettings();
  });

  els.foregroundSwatches.addEventListener("click", (event) => {
    const swatch = event.target.closest(".swatch");
    if (swatch !== null) {
      state.colorMode = "fg";
      applyColor(swatch.dataset.name);
    }
  });
  els.backgroundSwatches.addEventListener("click", (event) => {
    const swatch = event.target.closest(".swatch");
    if (swatch !== null) {
      state.colorMode = "bg";
      applyColor(swatch.dataset.name);
    }
  });
  els.linkApply.addEventListener("click", applyLink);
  els.linkAddress.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      applyLink();
    }
    if (event.key === "Escape") {
      closePopovers();
    }
  });

  document.addEventListener("mousedown", (event) => {
    if (
      event.target.closest(".popover") !== null ||
      event.target.closest(
        '[data-action="text-color"], [data-action="background-color"], [data-action="link"]',
      ) !== null
    ) {
      return;
    }
    closePopovers();
  });

  document.addEventListener("keydown", (event) => {
    const modifier = event.ctrlKey || event.metaKey;
    const typing =
      event.target instanceof HTMLElement &&
      event.target.closest("input, textarea, select, [contenteditable]") !==
        null;

    if (event.key === "Escape" && !els.settingsDialog.hidden) {
      event.preventDefault();
      closeSettings();
      return;
    }

    if (modifier && event.key.toLowerCase() === "s") {
      event.preventDefault();
      saveAll();
      return;
    }
    if (event.key === "/" && !typing) {
      event.preventDefault();
      els.search.focus();
      els.search.select();
      return;
    }
    if (event.key === "Escape") {
      closePopovers();
      if (document.activeElement === els.search) {
        els.search.value = "";
        state.search = "";
        renderList();
        els.search.blur();
      }
      return;
    }
    if (
      modifier &&
      document.activeElement === els.editor &&
      ["b", "i", "u", "e"].includes(event.key.toLowerCase())
    ) {
      event.preventDefault();
      applyAction(
        { b: "bold", i: "italic", u: "underline", e: "code" }[
          event.key.toLowerCase()
        ],
      );
    }
  });

  window.addEventListener("beforeunload", (event) => {
    if (anyDirty()) {
      event.preventDefault();
      event.returnValue = "";
    }
  });

  els.split.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    els.split.setPointerCapture(event.pointerId);
    els.split.classList.add("dragging");
  });
  els.split.addEventListener("pointermove", (event) => {
    if (!els.split.classList.contains("dragging")) {
      return;
    }
    const box = document.querySelector(".workspace").getBoundingClientRect();
    const ratio = Math.min(
      0.8,
      Math.max(0.2, (event.clientX - box.left) / box.width),
    );
    state.split = ratio;
    editorPane.style.flex = `0 0 calc((100% - 6px) * ${ratio})`;
    previewPane.style.flex = "1 1 0";
  });
  els.split.addEventListener("pointerup", (event) => {
    els.split.classList.remove("dragging");
    els.split.releasePointerCapture(event.pointerId);
    savePrefs();
  });
}

async function boot() {
  initSwatches();
  ensureCellPx();
  if (document.fonts !== undefined) {
    document.fonts.ready.then(() => {
      cellPx = null;
      ensureCellPx();
      renderPreview();
    });
  }
  bindEvents();
  editorPane.style.flex = `0 0 calc((100% - 6px) * ${state.split})`;
  previewPane.style.flex = "1 1 0";

  try {
    const response = await fetch("/api/state");
    if (!response.ok) {
      throw new Error("The editor server answered with an error.");
    }
    applyState(await response.json());
  } catch (error) {
    toast(error.message ?? "Could not read the copy", true);
  }
}

boot();
