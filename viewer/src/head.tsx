// Who an account is: picture, name, handle and FID, bio, and how many it
// follows and how many follow it. The author card shows it on a hover; the
// profile view at the top of the page. Information only, nothing to press.

import { countLabel, shownAccount } from "./author";
import { Avatar } from "./card";
import type { Profile } from "./reads";
import type { Account } from "./thread";

// `profile` is the account's profile once read, `null` where the read
// failed; until then the head shows what the page holds of `account`.
export function ProfileHead({ fid, account, profile }: {
  fid: number;
  account: Account | undefined;
  profile: Profile | null | undefined;
}) {
  const shown = shownAccount(fid, account, profile);
  const name = shown.display_name ?? shown.username ?? `fid ${fid}`;
  return (
    <>
      <div class="who">
        <Avatar account={shown} fid={fid} />
        <div class="names">
          <div class="name">{name}</div>
          <div class="handle">
            {shown.username && `@${shown.username} · `}fid {fid}
          </div>
        </div>
      </div>
      {profile?.bio && <p class="bio">{profile.bio}</p>}
      {profile ? (
        <p class="follows">
          <span>
            <strong>{countLabel(profile.following)}</strong> following
          </span>
          <span
            title={
              profile.followers_exact
                ? undefined
                : `at least ${profile.followers.toLocaleString("en-US")}: the page stops counting there`
            }
          >
            <strong>{countLabel(profile.followers, profile.followers_exact)}</strong> followers
          </span>
        </p>
      ) : (
        profile === undefined && <p class="follows reading">…</p>
      )}
    </>
  );
}
