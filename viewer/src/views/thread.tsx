// A thread: the casts above, the focus, and its direct replies, all of them
// or only the agent's picks.

import { type Actions, CastCard, type Role } from "../card";
import { count, repliesShown } from "../shown";
import { type Cast, type Thread, refOf } from "../thread";

type ThreadProps = { thread: Thread; only: string[] | null; onShowAll(): void } & Actions;

export function ThreadView({ thread, only, onShowAll, ...actions }: ThreadProps) {
  const accounts = new Map(thread.accounts.map((a) => [a.fid, a]));
  const now = Date.now();
  const card = (cast: Cast, role: Role) => (
    <CastCard key={cast.hash} cast={cast} role={role} accounts={accounts} now={now} {...actions} />
  );
  const all = count(thread.replies.length, "reply", "replies");
  const hidden = only ? thread.replies.length - only.length : 0;
  return (
    <>
      <Top top={thread.top} onOpen={actions.onOpen} />
      {thread.ancestors.map((c) => card(c, "ancestor"))}
      {card(thread.cast, "focus")}
      <h2 class="replies">{only ? `${only.length} of ${all}` : all}</h2>
      {repliesShown(thread, only).map((c) => card(c, "reply"))}
      {hidden > 0 && (
        <button class="note link" onClick={onShowAll}>
          Show all {all}
        </button>
      )}
    </>
  );
}

function Top({ top, onOpen }: { top: Thread["top"]; onOpen(ref: string): void }) {
  if (top === "root") return null;
  if ("missing" in top) {
    return <p class="note">Replies to a cast this node does not hold ({refOf(top.missing)}).</p>;
  }
  return (
    <button class="note link" onClick={() => onOpen(refOf(top.cut))}>
      Earlier casts in this thread…
    </button>
  );
}
