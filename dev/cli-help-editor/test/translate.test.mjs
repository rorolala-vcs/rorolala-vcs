import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SERVER = path.join(HERE, "..", "server.mjs");

function startMock() {
  const received = [];
  const server = http.createServer((request, response) => {
    let body = "";
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      received.push({
        url: request.url,
        headers: request.headers,
        body: JSON.parse(body),
      });
      response.writeHead(200, { "Content-Type": "text/event-stream" });
      response.write('data: {"choices":[{"delta":{"content":"你好"}}]}\n\n');
      response.write('data: {"choices":[{"delta":{"content":"，世界"}}]}\n\n');
      response.write("data: [DONE]\n\n");
      response.end();
    });
  });

  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      server.off("error", reject);
      resolve({
        port: server.address().port,
        received,
        close: () =>
          new Promise((done) => {
            server.close(done);
          }),
      });
    });
  });
}

function editorEnvironment(dataHome) {
  return {
    ...process.env,
    ROLA_DATA_HOME: dataHome,
    XDG_DATA_HOME: path.join(dataHome, "xdg-empty"),
    LOCALAPPDATA: path.join(dataHome, "local-empty"),
    HOME: path.join(dataHome, "home-empty"),
    USERPROFILE: path.join(dataHome, "home-empty"),
    CLI_HELP_EDITOR_NO_OPEN: "1",
  };
}

function startEditor(dataHome) {
  const child = spawn(process.execPath, [SERVER, "--no-open", "--port", "0"], {
    cwd: path.join(HERE, ".."),
    env: editorEnvironment(dataHome),
    stdio: ["ignore", "pipe", "pipe"],
  });

  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  return new Promise((resolve, reject) => {
    let output = "";
    const timer = setTimeout(() => {
      cleanup();
      reject(new Error(`the editor server did not start: ${stderr}`));
    }, 15000);
    const onData = (chunk) => {
      output += chunk.toString();
      const found = /http:\/\/127\.0\.0\.1:\d+\//u.exec(output);
      if (found !== null) {
        cleanup();
        resolve({ child, base: found[0].slice(0, -1) });
      }
    };
    const onExit = (code) => {
      cleanup();
      reject(new Error(`the editor server exited with ${code}: ${stderr}`));
    };
    const onError = (error) => {
      cleanup();
      reject(error);
    };
    const cleanup = () => {
      clearTimeout(timer);
      child.stdout.off("data", onData);
      child.off("exit", onExit);
      child.off("error", onError);
    };
    child.stdout.on("data", onData);
    child.on("exit", onExit);
    child.on("error", onError);
  });
}

function stopEditor(child) {
  return new Promise((resolve) => {
    if (child.exitCode !== null || child.signalCode !== null) {
      resolve();
      return;
    }
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      resolve();
    }, 3000);
    child.once("close", () => {
      clearTimeout(timer);
      resolve();
    });
    child.kill("SIGTERM");
  });
}

function writeConfig(dataHome, port) {
  const directory = path.join(dataHome, "rola", "dev");
  fs.mkdirSync(directory, { recursive: true });
  const file = path.join(directory, "llm-tools.toml");
  fs.writeFileSync(
    file,
    [
      "[translation]",
      `base_url = "http://127.0.0.1:${port}/v1"`,
      'api_key = "test-key"',
      'model = "test-model"',
      "temperature = 0",
      "timeout = 10",
    ].join("\n"),
  );
  return file;
}

test("the settings route writes what the translation reads", async () => {
  const mock = await startMock();
  const dataHome = fs.mkdtempSync(path.join(os.tmpdir(), "llm-tools-route-"));
  let editor = null;

  try {
    editor = await startEditor(dataHome);

    const before = await (await fetch(`${editor.base}/api/settings`)).json();
    assert.equal(
      before.file,
      path.join(dataHome, "rola", "dev", "llm-tools.toml"),
    );
    assert.equal(before.exists, false);
    assert.equal(before.error, null);
    assert.equal(before.config.baseUrl, "");

    const baseUrl = `http://127.0.0.1:${mock.port}/v1`;
    const posted = await fetch(`${editor.base}/api/settings`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        baseUrl,
        model: "test-model",
        apiKey: "test-key",
        temperature: 0.3,
        timeout: 15,
        headers: { "X-Test": "yes" },
      }),
    });
    assert.equal(posted.status, 200);
    const saved = await posted.json();
    assert.equal(saved.exists, true);
    assert.equal(saved.error, null);
    assert.equal(saved.config.model, "test-model");
    assert.equal(saved.config.headers["X-Test"], "yes");

    const translated = await fetch(`${editor.base}/api/translate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        program: "rola",
        key: "checkin.help",
        source: "en",
        target: "zh-CN",
        text: "Hello",
      }),
    });
    assert.equal(translated.status, 200);
    await translated.text();
    assert.equal(mock.received.length, 1);
    assert.equal(mock.received[0].headers.authorization, "Bearer test-key");
    assert.equal(mock.received[0].headers["x-test"], "yes");
    assert.equal(mock.received[0].body.temperature, 0.3);

    const refused = await fetch(`${editor.base}/api/settings`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ baseUrl: "not a url", model: "m" }),
    });
    assert.equal(refused.status, 400);
    assert.match((await refused.json()).error, /base_url/u);

    const after = await (await fetch(`${editor.base}/api/settings`)).json();
    assert.equal(after.config.baseUrl, baseUrl);
    assert.equal(after.config.timeout, 15);
  } finally {
    if (editor !== null) {
      await stopEditor(editor.child);
    }
    await mock.close();
    fs.rmSync(dataHome, { recursive: true, force: true });
  }
});

test("the translation route streams an OpenAI compatible answer", async () => {
  const mock = await startMock();
  const dataHome = fs.mkdtempSync(path.join(os.tmpdir(), "llm-tools-test-"));
  const config = writeConfig(dataHome, mock.port);
  let editor = null;

  try {
    editor = await startEditor(dataHome);
    const response = await fetch(`${editor.base}/api/translate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        program: "rola",
        key: "checkin.help",
        source: "en",
        target: "zh-CN",
        text: "Hello",
      }),
    });

    assert.equal(response.status, 200);
    assert.match(response.headers.get("content-type"), /text\/event-stream/u);
    const body = await response.text();
    const deltas = [...body.matchAll(/event: delta\ndata: (.*)\n\n/gu)].map(
      (match) => JSON.parse(match[1]).text,
    );
    assert.equal(deltas.join(""), "你好，世界");
    assert.match(body, /event: done/u);

    assert.equal(mock.received.length, 1);
    const request = mock.received[0];
    assert.equal(request.url, "/v1/chat/completions");
    assert.equal(request.headers.authorization, "Bearer test-key");
    assert.equal(request.body.model, "test-model");
    assert.equal(request.body.stream, true);
    assert.equal(request.body.messages[1].content, "Hello");
    assert.match(request.body.messages[0].content, /Simplified Chinese/u);

    fs.rmSync(config);
    const missing = await fetch(`${editor.base}/api/translate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        program: "rola",
        key: "checkin.help",
        source: "en",
        target: "zh-CN",
        text: "Hello",
      }),
    });
    assert.equal(missing.status, 503);
    const error = await missing.json();
    assert.match(error.error, /llm-tools\.toml/u);
  } finally {
    if (editor !== null) {
      await stopEditor(editor.child);
    }
    await mock.close();
    fs.rmSync(dataHome, { recursive: true, force: true });
  }
});
