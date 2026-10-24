// What an embedded link is, for its card: an image the page may draw inline,
// or a link named by its host.
//
// The page draws an image only from `https:`, and only where the page policy
// lets it load (`viewer serve --images`); a refused or broken one falls back
// to a link labelled "image". Each image loaded tells its host the owner's
// address and when they looked.

// An image: Farcaster's image host, or a link to an image file.
export function isImage(url: string): boolean {
  try {
    const u = new URL(url);
    return u.host === "imagedelivery.net" || /\.(png|jpe?g|gif|webp|avif)$/i.test(u.pathname);
  } catch {
    return false;
  }
}

// An image the page may try to draw: one served over https.
export const drawable = (url: string): boolean => isImage(url) && url.startsWith("https://");

export function hostOf(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./, "");
  } catch {
    return url;
  }
}

// The words an image stands in for: a cast's embed carries no description.
export const altOf = (url: string): string => `image from ${hostOf(url)}`;
