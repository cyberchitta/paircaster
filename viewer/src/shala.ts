// The page's side of Shala's hosting contract (Shala
// `skills/shala/references/hosting.md`): postMessage to the parent frame.
// Shala opens the page at a moniker (`reveal`), carries our CLI's messages to
// it (`tell`), records what the owner points at (`select`) and holds the
// view across restarts (`keep`, `recall`).

import type { Answer, Host } from "./host";

const RECALL_WAIT_MS = 300;

const post = (message: Record<string, unknown>) => window.parent.postMessage({ shala: 1, ...message }, "*");

// `project` is Shala's name for it, the page's own host there.
export function shalaHost(project: string): Host {
  return {
    project,
    opensLinks: false,
    connect: async (handlers) => {
      let recalled: (view: unknown) => void = () => {};
      window.addEventListener("message", async (event) => {
        if (event.source !== window.parent || event.data?.shala !== 1) return;
        const m = event.data;
        if ("recalled" in m) recalled(m.recalled);
        else if (m.reveal) post({ revealed: await settle(handlers.onReveal(m.reveal.moniker, m.reveal.state)) });
        else if (m.tell) post({ told: { id: m.tell.id, ...(await settle(handlers.onTell(m.tell.body))) } });
      });
      const kept = new Promise<unknown>((resolve) => {
        recalled = resolve;
        setTimeout(() => resolve(null), RECALL_WAIT_MS);
      });
      post({ recall: true });
      // Shala posts a reveal only after the page's first message other than
      // a recall, so this says the page can answer one.
      return { kept: await kept, start: () => post({}) };
    },
    select: (selection) => post({ select: selection }),
    keep: (view) => post({ keep: view }),
  };
}

async function settle(answer: Promise<Answer>): Promise<Answer> {
  try {
    return await answer;
  } catch (e) {
    return { ok: false, message: String(e instanceof Error ? e.message : e) };
  }
}
