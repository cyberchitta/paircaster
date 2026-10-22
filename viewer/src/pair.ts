// The page's side of pair-ui, the host that serves it in a plain browser
// without Shala: HTTP to its own origin, with the token from the URL's `pair`
// parameter. Commands reach the page only by tells, so opening a thread is a
// tell too (`{ selfcast: 1, open, state }`), handled as Shala's reveal is.

import type { Answer, Handlers, Host } from "./host";

const POLL_RETRY_MS = 2000;

export function pairHost(token: string, project: string): Host {
  const call = (method: string, route: string, body?: unknown) =>
    fetch(`/_pair/${route}`, {
      method,
      headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });

  async function poll(handlers: Handlers) {
    for (;;) {
      const response = await call("GET", "next").catch(() => null);
      if (!response?.ok) {
        await new Promise((r) => setTimeout(r, POLL_RETRY_MS));
        continue;
      }
      if (response.status === 204) continue;
      const { id, body } = await response.json();
      const answer = await settle(handle(handlers, body));
      await call("POST", "answer", { id, ...answer }).catch(() => {});
    }
  }

  return {
    project,
    opensLinks: true,
    connect: async (handlers) => {
      const kept = await call("GET", "recall")
        .then((r) => (r.ok ? r.json() : null))
        .catch(() => null);
      return { kept, start: () => void poll(handlers) };
    },
    select: () => {},
    keep: (view) => void call("PUT", "keep", view ?? null).catch(() => {}),
  };
}

function handle(handlers: Handlers, body: any): Promise<Answer> {
  if (typeof body?.open === "string") return handlers.onReveal(body.open, body.state ?? null);
  return handlers.onTell(body);
}

async function settle(answer: Promise<Answer>): Promise<Answer> {
  try {
    return await answer;
  } catch (e) {
    return { ok: false, message: String(e instanceof Error ? e.message : e) };
  }
}
