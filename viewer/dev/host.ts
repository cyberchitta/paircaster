// A minimal MCP Apps host for working on the viewer until Shala hosts it.
// It starts `selfcast mcp`, calls `view` the way a model would, and shows
// the viewer in a sandboxed frame. Its content policy is the spec's default
// plus what the page declares: the node in `csp.connectDomains` (in the
// spec) and WebAssembly for `csp.wasm` (not in the spec; the extension
// proposed to Shala). It passes on only the tool calls a page may make
// (visibility "app").
//
//   bun dev/host.ts [--node URL] [--port N]
//   then open http://127.0.0.1:5199/?cast=FID:HASH
//
// SELFCAST_BIN names the binary (default: ../target/debug/selfcast).

import { join } from "node:path";

const args = process.argv.slice(2);
const flag = (name: string) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const port = Number(flag("--port") ?? 5199);
const bin = process.env.SELFCAST_BIN ?? join(import.meta.dir, "../../target/debug/selfcast");
const nodeArgs = flag("--node") ? ["--node", flag("--node")!] : [];

const server = Bun.spawn([bin, ...nodeArgs, "mcp"], { stdin: "pipe", stdout: "pipe", stderr: "inherit" });
const waiting = new Map<number, (m: any) => void>();
let nextId = 1;

async function readReplies() {
  const decoder = new TextDecoder();
  let buffer = "";
  for await (const chunk of server.stdout) {
    buffer += decoder.decode(chunk, { stream: true });
    let nl: number;
    while ((nl = buffer.indexOf("\n")) >= 0) {
      const m = JSON.parse(buffer.slice(0, nl));
      buffer = buffer.slice(nl + 1);
      waiting.get(m.id)?.(m);
      waiting.delete(m.id);
    }
  }
}
readReplies();

function send(message: object) {
  server.stdin.write(JSON.stringify({ jsonrpc: "2.0", ...message }) + "\n");
  server.stdin.flush();
}

function rpc(method: string, params: unknown): Promise<any> {
  const id = nextId++;
  return new Promise((resolve) => {
    waiting.set(id, resolve);
    send({ id, method, params });
  });
}

const init = await rpc("initialize", {
  protocolVersion: "2025-11-25",
  capabilities: { extensions: { "io.modelcontextprotocol/ui": { mimeTypes: ["text/html;profile=mcp-app"] } } },
  clientInfo: { name: "selfcast-dev-host", version: "0.0.1" },
});
send({ method: "notifications/initialized" });
const tools: any[] = (await rpc("tools/list", {})).result.tools;
console.log(`selfcast mcp: protocol ${init.result.protocolVersion}, tools: ${tools.map((t) => t.name).join(", ")}`);

// The spec's default (apps.mdx:274-283), widened only by what the page
// declares.
function policy(csp: { connectDomains?: string[]; wasm?: boolean } = {}): string {
  const connect = csp.connectDomains?.length ? csp.connectDomains.join(" ") : "'none'";
  const wasm = csp.wasm ? " 'wasm-unsafe-eval'" : "";
  return (
    `default-src 'none'; script-src 'unsafe-inline'${wasm}; style-src 'unsafe-inline'; ` +
    `img-src data:; media-src data:; connect-src ${connect}`
  );
}

async function page(): Promise<string> {
  const tool = tools.find((t) => t.name === "view");
  const uri = tool._meta.ui.resourceUri;
  const content = (await rpc("resources/read", { uri })).result.contents[0];
  const csp = policy(content._meta?.ui?.csp);
  console.log(`page policy: ${csp}`);
  return content.text.replace("<head>", `<head><meta http-equiv="Content-Security-Policy" content="${csp}">`);
}

Bun.serve({
  hostname: "127.0.0.1",
  port,
  async fetch(req) {
    const url = new URL(req.url);
    if (url.pathname === "/") return new Response(HOST_PAGE, { headers: { "content-type": "text/html" } });
    if (url.pathname === "/api/page") return Response.json({ html: await page(), tools });
    if (url.pathname === "/api/call" && req.method === "POST") {
      const { name, arguments: a, from } = await req.json();
      const tool = tools.find((t) => t.name === name);
      const visibility: string[] = tool?._meta?.ui?.visibility ?? ["model", "app"];
      if (from === "app" && !visibility.includes("app")) {
        return Response.json({ error: { code: -32600, message: `${name} is not callable from the page` } });
      }
      return Response.json(await rpc("tools/call", { name, arguments: a }));
    }
    return new Response("not found", { status: 404 });
  },
});
console.log(`dev host: http://127.0.0.1:${port}/?cast=37:0xcd34bb7404d899df69fda5e8ca17d3f1aa1e9268`);

const HOST_PAGE = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>Selfcast dev host</title>
<style>
  body { margin: 0; font: 14px system-ui, sans-serif; display: grid; grid-template-columns: minmax(0, 1fr) 320px; height: 100vh; }
  iframe { width: 100%; height: 100%; border: 0; }
  aside { border-left: 1px solid #ddd; padding: 12px; overflow: auto; background: #fafafa; color: #222; }
  aside h3 { font-size: 12px; text-transform: uppercase; color: #777; margin: 12px 0 4px; }
  #context { white-space: pre-wrap; }
  #log { font: 11px ui-monospace, monospace; white-space: pre-wrap; color: #555; }
</style></head>
<body>
<iframe id="view" sandbox="allow-scripts"></iframe>
<aside>
  <h3>What the page told the model</h3><div id="context">nothing yet</div>
  <h3>Messages</h3><div id="log"></div>
</aside>
<script type="module">
const frame = document.getElementById("view");
const log = (dir, m) => {
  const line = dir + " " + (m.method ?? ("reply " + m.id));
  document.getElementById("log").prepend(line + "\\n");
};
const cast = new URLSearchParams(location.search).get("cast") ?? "37:0xcd34bb7404d899df69fda5e8ca17d3f1aa1e9268";
const call = (name, args, from) =>
  fetch("/api/call", { method: "POST", body: JSON.stringify({ name, arguments: args, from }) }).then((r) => r.json());
const { html } = await fetch("/api/page").then((r) => r.json());
const first = call("view", { cast }, "model");
const post = (m) => { log("→", m); frame.contentWindow.postMessage({ jsonrpc: "2.0", ...m }, "*"); };
const dark = matchMedia("(prefers-color-scheme: dark)").matches;

window.addEventListener("message", async (event) => {
  if (event.source !== frame.contentWindow) return;
  const m = event.data;
  log("←", m);
  switch (m.method) {
    case "ui/initialize":
      return post({ id: m.id, result: {
        protocolVersion: "2026-01-26",
        hostInfo: { name: "selfcast-dev-host", version: "0.0.1" },
        hostCapabilities: { serverTools: {}, openLinks: {}, updateModelContext: {} },
        hostContext: { theme: dark ? "dark" : "light", displayMode: "inline" },
      } });
    case "ui/notifications/initialized": {
      post({ method: "ui/notifications/tool-input", params: { arguments: { cast } } });
      const reply = await first;
      return post({ method: "ui/notifications/tool-result", params: reply.result ?? { isError: true, content: [{ type: "text", text: reply.error?.message }] } });
    }
    case "tools/call": {
      const reply = await call(m.params.name, m.params.arguments, "app");
      return post(reply.error ? { id: m.id, error: reply.error } : { id: m.id, result: reply.result });
    }
    case "ui/update-model-context":
      document.getElementById("context").textContent =
        (m.params.content ?? []).map((c) => c.text).join("\\n") + "\\n\\n" + JSON.stringify(m.params.structuredContent ?? {}, null, 1);
      return post({ id: m.id, result: {} });
    case "ui/open-link":
      window.open(m.params.url, "_blank", "noopener");
      return post({ id: m.id, result: {} });
    case "ui/notifications/size-changed":
      return;
    default:
      if (m.id !== undefined && m.method) post({ id: m.id, error: { code: -32601, message: "not supported here" } });
  }
});
frame.srcdoc = html;
</script>
</body></html>`;
