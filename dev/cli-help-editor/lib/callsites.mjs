// Where a translation key is used.
//
// The editor lists the copy, but a key on its own does not say what draws it or
// whether it is a help page. The Rust sources are read for `t!("key")` and for a
// constant `t!(KEY)`, together with the function around them, and the two facts
// are enough to show a call site and to mark a help handler.

const CONSTANT = /const\s+([A-Z0-9_]+)\s*:\s*&(?:'static\s+)?str\s*=\s*"([^"]*)"/;
const KEY_LITERAL = /t!\(\s*"([^"]+)"/;
const KEY_CONSTANT = /t!\(\s*([A-Z0-9_]+)\s*[,)]/;
const FUNCTION =
  /^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?(?:const\s+)?fn\s+([A-Za-z0-9_]+)/;

/** Adds `site` to `map[key]`, once per file and line. */
function remember(map, key, site) {
  const list = map.get(key) ?? [];
  if (!list.some((found) => found.file === site.file && found.line === site.line)) {
    list.push(site);
  }
  map.set(key, list);
}

/**
 * Reads the call sites out of the Rust sources.
 *
 * `files` are `{ path, text }`, with `path` relative to the project root. The
 * answer is a Map from a dotted key to the places it is named.
 */
export function scanCallsites(files) {
  const sites = new Map();

  for (const file of files) {
    const lines = file.text.split(/\r?\n/);
    const constants = new Map();
    for (const line of lines) {
      const found = CONSTANT.exec(line);
      if (found !== null) {
        constants.set(found[1], found[2]);
      }
    }

    let currentFunction = null;
    let currentHelp = false;
    for (let index = 0; index < lines.length; index += 1) {
      const line = lines[index];
      const functionMatch = FUNCTION.exec(line);
      if (functionMatch !== null) {
        currentFunction = functionMatch[1];
        currentHelp =
          currentFunction.startsWith("help") ||
          lines
            .slice(Math.max(0, index - 15), index)
            .some((above) => above.trim().startsWith("#[help"));
      }

      const literal = KEY_LITERAL.exec(line);
      if (literal !== null) {
        remember(sites, literal[1], {
          file: file.path,
          line: index + 1,
          function: currentFunction,
          help: currentHelp,
        });
        continue;
      }

      const constant = KEY_CONSTANT.exec(line);
      if (constant !== null) {
        const key = constants.get(constant[1]);
        if (key !== undefined) {
          remember(sites, key, {
            file: file.path,
            line: index + 1,
            function: currentFunction,
            help: currentHelp,
          });
        }
      }
    }
  }

  for (const [key, list] of sites) {
    list.sort((left, right) =>
      left.file === right.file ? left.line - right.line : left.file.localeCompare(right.file),
    );
    sites.set(key, list);
  }

  return sites;
}
