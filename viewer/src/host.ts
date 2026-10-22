// The page's side of MCP Apps (extension io.modelcontextprotocol/ui, spec
// 2026-01-26): JSON-RPC 2.0 with the host over postMessage. The page speaks
// first (`ui/initialize`), then receives the result of the tool it was
// opened by, and tells the model its current state.

export type ToolResult = {
  content?: { type: string; text?: string }[];
  structuredContent?: unknown;
  isError?: boolean;
};

export type HostContext = { theme?: "light" | "dark" };

export type Host = {
  callTool(name: string, args: Record<string, unknown>): Promise<ToolResult>;
  // Each call replaces the last: the model sees the page's current state,
  // not a history of it.
  setModelContext(text: string, structured: Record<string, unknown>): void;
  openLink(url: string): void;
};

export type Handlers = {
  onToolResult(result: ToolResult, host: Host): void;
  onContext(context: HostContext): void;
};

type Message = {
  id?: number | string;
  method?: string;
  params?: any;
  result?: any;
  error?: { code?: number; message: string };
};

const PROTOCOL_VERSION = "2026-01-26";

export function inHost(): boolean {
  return window.parent !== window;
}

export async function connect(handlers: Handlers): Promise<void> {
  const pending = new Map<number | string, (m: Message) => void>();
  let nextId = 1;
  const post = (m: Message) => window.parent.postMessage({ jsonrpc: "2.0", ...m }, "*");
  const request = (method: string, params: unknown) =>
    new Promise<any>((resolve, reject) => {
      const id = nextId++;
      pending.set(id, (m) => (m.error ? reject(new Error(m.error.message)) : resolve(m.result)));
      post({ id, method, params });
    });
  const host: Host = {
    callTool: (name, args) => request("tools/call", { name, arguments: args }),
    setModelContext: (text, structured) => {
      const params = { content: [{ type: "text", text }], structuredContent: structured };
      request("ui/update-model-context", params).catch(() => {});
    },
    openLink: (url) => {
      request("ui/open-link", { url }).catch(() => {});
    },
  };

  window.addEventListener("message", (event) => {
    if (event.source !== window.parent || event.data?.jsonrpc !== "2.0") return;
    const m: Message = event.data;
    if (m.method === undefined && m.id !== undefined) {
      pending.get(m.id)?.(m);
      pending.delete(m.id);
    } else if (m.method !== undefined) {
      receive(m, handlers, host, post);
    }
  });

  const init = await request("ui/initialize", {
    appInfo: { name: "selfcast-viewer", version: "0.0.1" },
    appCapabilities: {},
    protocolVersion: PROTOCOL_VERSION,
  });
  handlers.onContext(init?.hostContext ?? {});
  post({ method: "ui/notifications/initialized" });
  reportSize(post);
}

function receive(m: Message, handlers: Handlers, host: Host, post: (m: Message) => void) {
  switch (m.method) {
    case "ui/notifications/tool-result":
      return handlers.onToolResult(m.params, host);
    case "ui/notifications/host-context-changed":
      return handlers.onContext(m.params);
    case "ui/resource-teardown":
    case "ping":
      return post({ id: m.id, result: {} });
    default:
      if (m.id !== undefined) {
        post({ id: m.id, error: { code: -32601, message: `unknown method ${m.method}` } });
      }
  }
}

function reportSize(post: (m: Message) => void) {
  const send = () => {
    const el = document.documentElement;
    post({ method: "ui/notifications/size-changed", params: { width: el.scrollWidth, height: el.scrollHeight } });
  };
  new ResizeObserver(send).observe(document.body);
}
