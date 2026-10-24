// Casts from anywhere, in the order the moniker names them, each reply
// naming the cast it replies to.

import { type Actions, CastCard } from "../card";
import { count } from "../shown";
import type { Account, Cast } from "../thread";

export function ListView({ casts, accounts, ...actions }: { casts: Cast[]; accounts: Account[] } & Actions) {
  const byFid = new Map(accounts.map((a) => [a.fid, a]));
  const now = Date.now();
  return (
    <>
      <h2 class="replies">{count(casts.length, "cast")}</h2>
      {casts.map((c) => (
        <CastCard key={c.hash} cast={c} role="reply" replying accounts={byFid} now={now} {...actions} />
      ))}
    </>
  );
}
