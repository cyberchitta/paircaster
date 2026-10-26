// An account: who it is, then its latest casts, newest first, as a list
// draws them, each reply naming the cast it replies to.

import { type Actions, CastCard } from "../card";
import { ProfileHead } from "../head";
import type { Profile } from "../reads";
import { count } from "../shown";
import type { Account, Cast } from "../thread";

export function ProfileView({
  profile,
  casts,
  accounts,
  ...actions
}: {
  profile: Profile;
  casts: readonly Cast[];
  accounts: readonly Account[];
} & Actions) {
  const byFid = new Map(accounts.map((a) => [a.fid, a]));
  const now = Date.now();
  return (
    <>
      <section class="profile">
        <ProfileHead fid={profile.fid} account={byFid.get(profile.fid)} profile={profile} />
      </section>
      <h2 class="replies">{casts.length > 0 ? count(casts.length, "latest cast") : "No casts on this node"}</h2>
      {casts.map((c) => (
        <CastCard key={c.hash} cast={c} role="reply" replying accounts={byFid} now={now} {...actions} />
      ))}
    </>
  );
}
