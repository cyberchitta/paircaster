// The author card: resting the pointer on an author's picture or name, or
// tabbing to the name, shows who they are: picture, name, handle, bio, and
// how many accounts they follow and how many follow them. It is information
// only, with nothing in it to press; a click on the author still does what a
// click on the cast does.
//
// One card shows at a time, drawn over the page outside its cards. The page
// reads each profile once per node, through `want`, and hands back what it
// holds in `profiles`; until a read lands, or where it fails, the card shows
// what the page already knows.

import { type ComponentChildren, createContext } from "preact";
import { useContext, useEffect, useLayoutEffect, useMemo, useRef, useState } from "preact/hooks";

import { CLOSE_DELAY, HOVER_DELAY, type Profiles, type Rect, countLabel, place, shownAccount } from "./author";
import { Avatar } from "./card";
import type { Account } from "./thread";

type Hovered = { fid: number; account: Account | undefined; el: Element; anchor: Rect };

type Hub = {
  enter(fid: number, account: Account | undefined, el: Element, delay: number): void;
  leave(): void;
  stay(): void;
};

const HubContext = createContext<Hub | null>(null);

// Holds the card for the page within it; `want` asks the page to read an
// account's profile.
export function Hover({ profiles, want, children }: {
  profiles: Profiles;
  want(fid: number): void;
  children: ComponentChildren;
}) {
  const [hovered, setHovered] = useState<Hovered | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const wantRef = useRef(want);
  wantRef.current = want;
  // What the card describes now, so moving from a name to its handle or
  // picture on the same cast keeps the card instead of reopening it.
  const shown = useRef<Hovered | null>(null);
  shown.current = hovered;
  const hub = useMemo<Hub>(() => {
    const later = (f: () => void, ms: number) => {
      clearTimeout(timer.current);
      timer.current = setTimeout(f, ms);
    };
    return {
      enter: (fid, account, el, delay) => {
        if (shown.current && sameCast(shown.current, fid, el)) return clearTimeout(timer.current);
        later(() => {
          if (!el.isConnected) return;
          setHovered({ fid, account, el, anchor: el.getBoundingClientRect() });
          wantRef.current(fid);
        }, delay);
      },
      leave: () => later(() => setHovered(null), CLOSE_DELAY),
      stay: () => clearTimeout(timer.current),
    };
  }, []);

  // The card describes what was under the pointer: Escape, a scroll or a
  // resize puts it away.
  useEffect(() => {
    if (!hovered) return;
    const close = () => (clearTimeout(timer.current), setHovered(null));
    const key = (e: KeyboardEvent) => e.key === "Escape" && close();
    addEventListener("keydown", key);
    addEventListener("scroll", close, { capture: true, passive: true });
    addEventListener("resize", close);
    return () => {
      removeEventListener("keydown", key);
      removeEventListener("scroll", close, { capture: true });
      removeEventListener("resize", close);
    };
  }, [hovered]);
  useEffect(() => () => clearTimeout(timer.current), []);
  // A new view drew over the author the card described.
  useEffect(() => {
    if (hovered && !hovered.el.isConnected) setHovered(null);
  });

  return (
    <HubContext.Provider value={hub}>
      {children}
      {hovered && <Card hovered={hovered} profile={profiles.get(hovered.fid)} hub={hub} />}
    </HubContext.Provider>
  );
}

const CARD = "article, .quote";
const sameCast = (h: Hovered, fid: number, el: Element) => h.fid === fid && h.el.closest(CARD) === el.closest(CARD);

// What an author's picture or name takes to open the card: a pointer that
// rests on it (not a touch, which is a click), or keyboard focus on it.
export function useAuthor(fid: number, account: Account | undefined) {
  const hub = useContext(HubContext);
  if (!hub) return {};
  return {
    onPointerEnter: (e: PointerEvent) =>
      e.pointerType !== "touch" && hub.enter(fid, account, e.currentTarget as Element, HOVER_DELAY),
    onPointerLeave: () => hub.leave(),
    onFocus: (e: FocusEvent) => {
      const el = e.currentTarget as Element;
      if (el.matches(":focus-visible")) hub.enter(fid, account, el, 0);
    },
    onBlur: () => hub.leave(),
  };
}

function Card({ hovered, profile, hub }: { hovered: Hovered; profile: ReturnType<Profiles["get"]>; hub: Hub }) {
  const { fid, anchor } = hovered;
  const box = useRef<HTMLDivElement>(null);
  const [spot, setSpot] = useState<{ top: number; left: number } | null>(null);
  const account = shownAccount(fid, hovered.account, profile);
  // Placed once drawn, when its size is known, and again as the profile
  // fills it in.
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const pane = { width: document.documentElement.clientWidth, height: document.documentElement.clientHeight };
    setSpot(place(anchor, { width: el.offsetWidth, height: el.offsetHeight }, pane));
  }, [anchor, profile]);
  const name = account.display_name ?? account.username ?? `fid ${fid}`;
  return (
    <div
      ref={box}
      class="hovercard"
      role="tooltip"
      style={spot ? { top: `${spot.top}px`, left: `${spot.left}px` } : { visibility: "hidden" }}
      onPointerEnter={() => hub.stay()}
      onPointerLeave={() => hub.leave()}
    >
      <div class="who">
        <Avatar account={account} fid={fid} />
        <div class="names">
          <div class="name">{name}</div>
          <div class="handle">
            {account.username && `@${account.username} · `}fid {fid}
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
    </div>
  );
}
