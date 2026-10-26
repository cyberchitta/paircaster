// A thread: the casts above, the focus, and its replies, nested under one
// another as deep as the page nests, with long runs folded ("+8 more")
// until clicked. With the agent's picks (`only`), the picks come first, in
// its order, and the other replies fold below them.

import { type Actions, Avatar, CastCard, type Role } from "../card";
import { type FoldKey, layout, type Piece } from "../fold";
import { accountsOf, count } from "../shown";
import { type Account, type Cast, refOf, type Thread } from "../thread";

type ThreadProps = {
  thread: Thread;
  only: string[] | null;
  opened: ReadonlySet<string>;
  depth: number;
  onUnfold(fold: FoldKey): void;
} & Actions;

export function ThreadView({ thread, only, opened, depth, onUnfold, ...actions }: ThreadProps) {
  const nest = { branches: actions.branches, depth };
  const accounts = new Map(accountsOf({ kind: "thread", thread, only }, nest).map((a) => [a.fid, a]));
  const now = Date.now();
  const { above, replies } = layout(thread, only, opened, actions.marked, nest);
  // `level` is how deep a reply is: 1 for the focus's own.
  const piece = (p: Piece, role: Role, level: number) => {
    if ("fold" in p) {
      return (
        <Fold
          key={p.fold}
          casts={p.casts}
          role={role}
          nested={level > 1}
          label={labelOf(p.fold, p.casts.length, only)}
          accounts={accounts}
          onOpen={() => onUnfold(p.fold)}
        />
      );
    }
    const under = p.replies.length > 0 || p.continues;
    const card = <CastCard key={p.cast.hash} cast={p.cast} role={role} nested={level > 1} parent={under} accounts={accounts} now={now} {...actions} />;
    if (!under) return card;
    return [
      card,
      <div key={`${p.cast.hash}/replies`} class="nest">
        {p.replies.map((r) => piece(r, "reply", level + 1))}
        {p.continues && (
          <button type="button" class="continue" onClick={() => actions.onOpen(refOf(p.cast))}>
            Continue this thread →
          </button>
        )}
      </div>,
    ];
  };
  const all = count(thread.replies.length, "reply", "replies");
  return (
    <>
      <Top top={thread.top} onOpen={actions.onOpen} />
      {above.map((p) => piece(p, "ancestor", 0))}
      {piece({ cast: thread.cast, replies: [], continues: false }, "focus", 0)}
      <h2 class="replies">{only ? `${only.length} of ${all}` : all}</h2>
      {replies.map((p) => piece(p, "reply", 1))}
    </>
  );
}

function labelOf(fold: FoldKey, n: number, only: string[] | null): string {
  if (fold === "above") return `+${count(n, "more cast")}`;
  return only && fold === "replies" ? `+${count(n, "other reply", "other replies")}` : `+${n} more`;
}

// A folded run: its size and the first few authors; a click opens it in place.
function Fold(props: { casts: readonly Cast[]; role: Role; nested: boolean; label: string; accounts: Map<number, Account>; onOpen(): void }) {
  const { casts, role, nested, label, accounts, onOpen } = props;
  const authors = [...new Set(casts.map((c) => c.fid))].slice(0, 3);
  return (
    <button type="button" class={`fold ${role}${nested ? " nested" : ""}`} onClick={onOpen} title={`show ${count(casts.length, "cast")}`}>
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
    <button type="button" class="note link" onClick={() => onOpen(refOf(top.cut))}>
      Earlier casts in this thread…
    </button>
  );
}
