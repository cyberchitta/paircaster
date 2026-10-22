// What the viewer needs from whatever hosts it: tells in, answers out, one
// kept value, and a place to report what the owner points at. Shala carries
// these over postMessage (`shala.ts`); pair-ui, in a plain browser, over
// HTTP (`pair.ts`). The viewer above this is the same in both.

import { pairHost } from "./pair";
import { shalaHost } from "./shala";

export type Answer = { ok: boolean; message: string; result?: unknown };

export type Handlers = {
  onReveal(moniker: string, state: unknown): Promise<Answer>;
  onTell(body: unknown): Promise<Answer>;
};

export type Selection = { moniker: string; label: string; state?: unknown };

export type Host = {
  // The project monikers name.
  project: string;
  // Whether a link can open in a new tab: Shala's sandbox opens none.
  opensLinks: boolean;
  // Listens, and gives back what was kept. `start` says the page can now
  // answer; call it once the kept view is shown.
  connect(handlers: Handlers): Promise<{ kept: unknown; start(): void }>;
  // Only for what the owner did, never for what a tell or reveal did.
  select(selection: Selection | null): void;
  keep(view: unknown): void;
};

// The project the CLI's monikers name outside Shala.
const PROJECT = "selfcast";

export function findHost(): Host | null {
  const token = new URLSearchParams(location.search).get("pair");
  if (token) return pairHost(token, PROJECT);
  if (window.parent !== window) return shalaHost(location.host);
  return null;
}
