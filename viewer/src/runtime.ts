// What a page built on Selfcast can use: the reads, named after the CLI and
// returning what `selfcast <read> --json` prints, run in the page through
// selfcast-core compiled to WebAssembly; and the host bridge, reduced to
// what a page needs. Reads only: a page holds no keys and cannot write.
//
// The viewer is the first page built on this. A page an agent writes would
// use the same object, found at `window.selfcast`.

import { initSync, thread as wasmThread } from "../wasm/selfcast_wasm.js";
import wasmBase64 from "../wasm/module";
import type { Host } from "./host";
import { type Thread, asThread } from "./thread";

export type Selfcast = {
  // The node the page reads, as the host was told.
  node: string;
  // `selfcast thread <cast> --limit <limit> --json`.
  thread(cast: string, limit?: number): Promise<Thread>;
  // Replaces what the model is told about the page: its current state, as
  // one line of text and as data.
  setState(text: string, state: Record<string, unknown>): void;
  openLink(url: string): void;
};

let loaded = false;

function load() {
  if (loaded) return;
  const bytes = Uint8Array.from(atob(wasmBase64), (c) => c.charCodeAt(0));
  initSync({ module: bytes });
  loaded = true;
}

export function makeRuntime(host: Host, node: string): Selfcast {
  load();
  const now = () => Math.floor(Date.now() / 1000);
  const selfcast: Selfcast = {
    node,
    thread: async (cast, limit = 100) => asThread(JSON.parse(await wasmThread(node, cast, limit, now()))),
    setState: (text, state) => host.setModelContext(text, state),
    openLink: (url) => host.openLink(url),
  };
  (window as any).selfcast = selfcast;
  return selfcast;
}
