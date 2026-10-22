// The page's side of Shala's hosting contract (Shala
// `skills/shala/references/hosting.md`): pair-ui's messages, keyed `pair: 1`,
// over postMessage to the parent frame. Shala opens the page at a moniker
// (`reveal`), carries our CLI's messages to it (`tell`), and holds the view
// across restarts (`keep`, `recall`). Both reveals and tells are answered
// with `answer`, by id.

import type { Answer, Host } from "./host";

const RECALL_WAIT_MS = 300;

const post = (message: Record<string, unknown>) => window.parent.postMessage({ pair: 1, ...message }, "*");

// `project` is Shala's name for it, the page's own host there.
export function shalaHost(project: string): Host {
  return {
    project,
    opensLinks: false,
    connect: async (handlers) => {
      let recalled: (view: unknown) => void = () => {};
      window.addEventListener("message", async (event) => {
        if (event.source !== window.parent || event.data?.pair !== 1) return;
        const m = event.data;
        if ("recalled" in m) recalled(m.recalled);
        else if (m.reveal) answer(m.reveal.id, handlers.onReveal(m.reveal.moniker, m.reveal.state));
        else if (m.tell) answer(m.tell.id, handlers.onTell(m.tell.body));
      });
      const kept = new Promise<unknown>((resolve) => {
        recalled = resolve;
        setTimeout(() => resolve(null), RECALL_WAIT_MS);
      });
      post({ recall: true });
      // Shala holds reveals until this arrives.
      return { kept: await kept, start: () => post({ ready: true }) };
    },
    keep: (view) => post({ keep: view }),
  };
}

async function answer(id: unknown, pending: Promise<Answer>) {
  post({ answer: { id, ...(await settle(pending)) } });
}

async function settle(answer: Promise<Answer>): Promise<Answer> {
  try {
    return await answer;
  } catch (e) {
    return { ok: false, message: String(e instanceof Error ? e.message : e) };
  }
}
