// A cast's direct replies, as one `replies` read gives them. The node stores
// no reply counts, so this one read is both the count on the cast's card and
// the casts that open under it; the page keeps it per cast.

import type { Replies } from "./reads";
import { count } from "./shown";
import type { Account, Cast, Thread } from "./thread";

// How many replies a count names exactly; past it the card says "100+".
export const COUNTED = 100;
// What a branch read asks for: one past what is counted, to tell 100 from more.
export const BRANCH_READ = COUNTED + 1;
// What the `thread` read holds of the focus's replies (its default limit).
const THREAD_READ = 100;

export type Branch = { casts: readonly Cast[]; accounts: readonly Account[] };

export const branchOf = (r: Replies): Branch => ({ casts: r.casts, accounts: r.accounts });

// The focus's replies, when the thread read holds all of them; else the
// focus is read like any other cast.
export const seedOf = (thread: Thread): Branch | null => (thread.replies.length < THREAD_READ ? { casts: thread.replies, accounts: thread.accounts } : null);

// What a card says of its replies; nothing for none.
export function repliesLabel(branch: Branch): string | null {
  const n = branch.casts.length;
  if (n === 0) return null;
  return n > COUNTED ? `${COUNTED}+ replies` : count(n, "reply", "replies");
}
