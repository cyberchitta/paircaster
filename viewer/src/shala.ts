// The page's side of Shala's hosting contract (Shala
// `skills/shala/references/hosting.md`): postMessage to the parent frame.
// Shala opens the page at a moniker (`reveal`), carries our CLI's messages to
// it (`tell`), records what the owner points at (`select`) and holds the
// view across restarts (`keep`, `recall`).

export type Answer = { ok: boolean; message: string; result?: unknown };

export type Handlers = {
  onReveal(moniker: string, state: unknown): Promise<Answer>;
  onTell(body: unknown): Promise<Answer>;
};

export type Selection = { moniker: string; label: string; state?: unknown };

const RECALL_WAIT_MS = 300;

export const inFrame = () => window.parent !== window;

const post = (message: Record<string, unknown>) => window.parent.postMessage({ shala: 1, ...message }, "*");

// Listens, and returns what Shala kept for us: null when nothing was kept, or
// when Shala does not answer in time.
export function connect(handlers: Handlers): Promise<unknown> {
  let recalled: (view: unknown) => void = () => {};
  window.addEventListener("message", async (event) => {
    if (event.source !== window.parent || event.data?.shala !== 1) return;
    const m = event.data;
    if ("recalled" in m) recalled(m.recalled);
    else if (m.reveal) post({ revealed: await settle(handlers.onReveal(m.reveal.moniker, m.reveal.state)) });
    else if (m.tell) post({ told: { id: m.tell.id, ...(await settle(handlers.onTell(m.tell.body))) } });
  });
  const answer = new Promise<unknown>((resolve) => {
    recalled = resolve;
    setTimeout(() => resolve(null), RECALL_WAIT_MS);
  });
  post({ recall: true });
  return answer;
}

async function settle(answer: Promise<Answer>): Promise<Answer> {
  try {
    return await answer;
  } catch (e) {
    return { ok: false, message: String(e instanceof Error ? e.message : e) };
  }
}

// Shala posts a reveal only after the page's first message other than a
// recall, so this says the page can answer one.
export const ready = () => post({});

// Only for what the owner did: a reveal or a tell must not become the
// owner's selection.
export const select = (selection: Selection | null) => post({ select: selection });

export const keep = (view: unknown) => post({ keep: view });
