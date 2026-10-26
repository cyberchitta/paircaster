import type { Actions } from "../card";
import type { FoldKey } from "../fold";
import type { Shown } from "../shown";
import { ListView } from "./list";
import { ProfileView } from "./profile";
import { ThreadView } from "./thread";

interface ShownProps extends Actions {
  shown: Shown;
  opened: ReadonlySet<string>;
  depth: number;
  onUnfold(fold: FoldKey): void;
}

// The view for what the page shows: a thread, a profile or a list of casts.
export function ShownView({ shown, opened, depth, onUnfold, ...actions }: ShownProps) {
  switch (shown.kind) {
    case "thread":
      return <ThreadView thread={shown.thread} only={shown.only} opened={opened} depth={depth} onUnfold={onUnfold} {...actions} />;
    case "profile":
      return <ProfileView profile={shown.profile} casts={shown.casts} accounts={shown.accounts} {...actions} />;
    case "casts":
      return <ListView casts={shown.casts} accounts={shown.accounts} {...actions} />;
    default: {
      const unknown: never = shown;
      throw new Error(`no view for ${JSON.stringify(unknown)}`);
    }
  }
}
