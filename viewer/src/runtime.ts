// What a page built on Selfcast can use: the reads, named after the CLI and
// returning what `selfcast <read> --json` prints, run in the page through
// selfcast-core compiled to WebAssembly. Reads only: a page holds no keys
// and cannot write.
//
// The viewer is the first page built on this. A page an agent writes would
// use the same object, found at `window.selfcast`.

import { initSync, thread as wasmThread } from "../wasm/selfcast_wasm.js";
import wasmBase64 from "../wasm/module";
import { type Thread, asThread } from "./thread";

export type Selfcast = {
  // The node the page reads.
  node: string;
  // `selfcast thread <cast> --limit <limit> --json`.
  thread(cast: string, limit?: number): Promise<Thread>;
};

let loaded = false;

function load() {
  if (loaded) return;
  const bytes = Uint8Array.from(atob(wasmBase64), (c) => c.charCodeAt(0));
  initSync({ module: bytes });
  loaded = true;
}

export function makeRuntime(node: string): Selfcast {
  load();
  const now = () => Math.floor(Date.now() / 1000);
  const selfcast: Selfcast = {
    node,
    thread: async (cast, limit = 100) => asThread(JSON.parse(await wasmThread(node, cast, limit, now()))),
  };
  (window as any).selfcast = selfcast;
  return selfcast;
}
