#!/usr/bin/env node
// The editor's server.
//
// It serves one local page, reads the i18n tree into one state, and writes a batch
// of edits back. Nothing here decides what a message looks like: the browser runs
// the same renderer and the same checks, and the server only keeps the files and
// the source positions.

import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

import {
  EditorError,
  PROGRAM_DIRECTORIES,
  findRoot,
  readState,
  writeEdits,
} from "./lib/scan.mjs";
import {
  readSettings,
  translateStream,
  writeSettings,
} from "./lib/translate.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PUBLIC = path.join(HERE, "public");
const LIB = path.join(HERE, "lib");
const LIMIT = 20 * 1024 * 1024;

const MIME = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".svg": "image/svg+xml",
  ".json": "application/json; charset=utf-8",
  ".png": "image/png",
  ".ico": "image/x-icon",
};

function argument(name) {
  const at = process.argv.indexOf(name);
  return at < 0 ? null : (process.argv[at + 1] ?? null);
}

function flag(name) {
  return process.argv.includes(name);
}

function json(response, status, value) {
  const body = JSON.stringify(value);
  response.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Content-Length": Buffer.byteLength(body),
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
  });
  response.end(body);
}

function text(response, status, value) {
  const body = String(value);
  response.writeHead(status, {
    "Content-Type": "text/plain; charset=utf-8",
    "Content-Length": Buffer.byteLength(body),
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
  });
  response.end(body);
}

function readBody(request) {
  return new Promise((resolve, reject) => {
    let size = 0;
    const parts = [];
    request.on("data", (chunk) => {
      size += chunk.length;
      if (size > LIMIT) {
        reject(new EditorError("the request is too large", 413));
        request.destroy();
        return;
      }
      parts.push(chunk);
    });
    request.on("end", () => resolve(Buffer.concat(parts).toString("utf8")));
    request.on("error", reject);
  });
}

function inside(directory, candidate) {
  const relative = path.relative(directory, candidate);
  return (
    relative !== "" && !relative.startsWith("..") && !path.isAbsolute(relative)
  );
}

function staticFile(response, pathname) {
  const decoded = decodeURIComponent(pathname);
  const target = decoded.startsWith("/lib/")
    ? path.resolve(LIB, decoded.slice("/lib/".length))
    : path.resolve(PUBLIC, decoded === "/" ? "index.html" : decoded.slice(1));

  if (!inside(LIB, target) && !inside(PUBLIC, target)) {
    text(response, 404, "not found");
    return;
  }

  let content;
  try {
    content = fs.readFileSync(target);
  } catch {
    text(response, 404, "not found");
    return;
  }

  const type =
    MIME[path.extname(target).toLowerCase()] ?? "application/octet-stream";
  response.writeHead(200, {
    "Content-Type": type,
    "Content-Length": content.length,
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
  });
  response.end(content);
}

function browser(url) {
  if (
    flag("--no-open") ||
    process.env.CI ||
    process.env.CLI_HELP_EDITOR_NO_OPEN
  ) {
    return;
  }
  const command =
    process.platform === "win32"
      ? ["cmd", ["/c", "start", "", url]]
      : process.platform === "darwin"
        ? ["open", [url]]
        : ["xdg-open", [url]];
  const child = spawn(command[0], command[1], {
    detached: true,
    stdio: "ignore",
    windowsHide: true,
  });
  child.on("error", () => {
    // The address was printed; a browser that cannot be started is not a failure.
  });
  child.unref();
}

async function listen(port) {
  return new Promise((resolve, reject) => {
    const server = http.createServer();
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      server.off("error", reject);
      resolve(server);
    });
  });
}

async function main() {
  const major = Number(process.versions.node.split(".")[0]);
  if (major < 20) {
    console.error("Node.js 20 or newer is required.");
    process.exit(1);
  }

  const root = findRoot(argument("--root") ?? undefined);
  const preferred = Number(
    argument("--port") ?? process.env.CLI_HELP_EDITOR_PORT ?? 7317,
  );
  let server = null;
  let port = 0;

  for (let offset = 0; offset <= 10; offset += 1) {
    const candidate =
      Number.isFinite(preferred) && preferred > 0 ? preferred + offset : 0;
    try {
      server = await listen(candidate);
      port = server.address().port;
      break;
    } catch (error) {
      if (offset === 10 && preferred > 0) {
        server = await listen(0);
        port = server.address().port;
        break;
      }
      if (error.code !== "EADDRINUSE") {
        throw error;
      }
    }
  }

  if (server === null) {
    throw new Error("no local port could be bound");
  }

  server.on("request", async (request, response) => {
    try {
      const url = new URL(
        request.url ?? "/",
        `http://${request.headers.host ?? "127.0.0.1"}`,
      );
      const pathname = url.pathname;

      if (request.method === "GET" && pathname === "/api/state") {
        json(response, 200, readState(root));
        return;
      }

      if (request.method === "POST" && pathname === "/api/save") {
        const body = JSON.parse(await readBody(request));
        if (body === null || typeof body !== "object") {
          throw new EditorError("the request is not an object");
        }
        const state = writeEdits(root, body.edits);
        json(response, 200, state);
        return;
      }

      if (request.method === "POST" && pathname === "/api/translate") {
        const body = JSON.parse(await readBody(request));
        if (body === null || typeof body !== "object") {
          throw new EditorError("the request is not an object");
        }
        await translateStream(body, request, response);
        return;
      }

      if (request.method === "GET" && pathname === "/api/settings") {
        json(response, 200, readSettings());
        return;
      }

      if (request.method === "POST" && pathname === "/api/settings") {
        const body = JSON.parse(await readBody(request));
        if (body === null || typeof body !== "object") {
          throw new EditorError("the request is not an object");
        }
        json(response, 200, writeSettings(body));
        return;
      }

      if (request.method === "GET" && pathname === "/api/reload") {
        json(response, 200, readState(root));
        return;
      }

      if (request.method === "GET" && pathname === "/favicon.ico") {
        response.writeHead(204);
        response.end();
        return;
      }

      if (request.method === "GET") {
        staticFile(response, pathname);
        return;
      }

      text(response, 405, "method not allowed");
    } catch (error) {
      if (response.destroyed) {
        return;
      }
      if (error instanceof EditorError) {
        json(response, error.status, { error: error.message });
      } else if (error instanceof SyntaxError) {
        json(response, 400, { error: "the request is not JSON" });
      } else {
        console.error(error);
        json(response, 500, { error: error.message ?? "internal error" });
      }
    }
  });

  const url = `http://127.0.0.1:${port}/`;
  const editing = PROGRAM_DIRECTORIES.map((directory) =>
    path.join(directory, "i18n"),
  )
    .filter((directory) => fs.existsSync(path.join(root, directory)))
    .map((directory) => directory.split(path.sep).join("/"));
  console.log(`CLI help editor: ${url}`);
  console.log(`Editing: ${editing.join(", ")}`);
  console.log("Press Ctrl+C to stop.");
  browser(url);

  const stop = () => {
    server.closeAllConnections?.();
    server.close(() => process.exit(0));
    setTimeout(() => process.exit(0), 500).unref();
  };
  process.on("SIGINT", stop);
  process.on("SIGTERM", stop);
}

main().catch((error) => {
  console.error(error.message ?? error);
  process.exit(1);
});
