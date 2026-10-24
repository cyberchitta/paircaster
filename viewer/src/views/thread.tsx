// A thread: the casts above, the focus, and its direct replies, with long
// runs folded ("+8 more") until clicked. With the agent's picks (`only`),
// the picks come first, in its order, and the other replies fold below them.

import { type Actions, Avatar, CastCard, type Role } from "../card";
import { type FoldKey, type Piece, layout } from "../fold";
import { count } from "../shown";
import { type Account, type Cast, type Thread, refOf } from "../thread";

type ThreadProps = {
  thread: Thread;
  only: string[] | null;
  opened: ReadonlySet<string>;
  onUnfold(fold: FoldKey): void;
} & Actions;

export function ThreadView({ thread, only, opened, onUnfold, ...actions }: ThreadProps) {
  const accounts = new Map(thread.accounts.map((a) => [a.fid, a]));
  const now = Date.now();
  const { above, replies } = layout(thread, only, opened, actions.marked);
  const piece = (p: Piece, role: Role) =>
    "cast" in p ? (
      <CastCard key={p.cast.hash} cast={p.cast} role={role} accounts={accounts} now={now} {...actions} />
    ) : (
      <Fold
        key={p.fold}
        casts={p.casts}
        role={role}
        label={labelOf(p.fold, p.casts.length, only)}
        accounts={accounts}
        onOpen={() => onUnfold(p.fold)}
      />
    );
  const all = count(thread.replies.length, "reply", "replies");
  return (
    <>
      <Top top={thread.top} onOpen={actions.onOpen} />
      {above.map((p) => piece(p, "ancestor"))}
      {piece({ cast: thread.cast }, "focus")}
      <h2 class="replies">{only ? `${only.length} of ${all}` : all}</h2>
      {replies.map((p) => piece(p, "reply"))}
    </>
  );
}

function labelOf(fold: FoldKey, n: number, only: string[] | null): string {
  if (fold === "above") return `+${count(n, "more cast")}`;
  return only ? `+${count(n, "other reply", "other replies")}` : `+${n} more`;
}

// A folded run: its size and the first few authors; a click opens it in place.
function Fold(props: { casts: Cast[]; role: Role; label: string; accounts: Map<number, Account>; onOpen(): void }) {
  const { casts, role, label, accounts, onOpen } = props;
  const authors = [...new Set(casts.map((c) => c.fid))].slice(0, 3);
  return (
    <button class={`fold ${role}`} onClick={onOpen} title={`show ${count(casts.length, "cast")}`}>
      <span class="rail">{role === "ancestor" && <span class="line dotted" />}</span>
      <span class="faces" aria-hidden="true">
        {authors.map((fid) => (
          <Avatar key={fid} account={accounts.get(fid)} fid={fid} />
        ))}
      </span>
      <span class="label">{label}</span>
    </button>
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
