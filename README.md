# paircaster: site-build

The toolchain that builds the Paircaster site. The pages themselves are
markdown in `docs/` on `main`; this branch holds only what turns them into
HTML, so `main` carries no JavaScript dependencies.

The workflow on `main` (`.github/workflows/pages.yml`) checks out this
branch, copies `main`'s `docs/` into `content/`, and runs `bun run build`.
A push here runs nothing: after changing the toolchain, run that workflow
by hand.

Styling comes from Supramental Gold, pinned in `package.json`; its CSS is
fetched from jsDelivr at the pinned tag.

Pages use three boxes, `<warning>`, `<limit>` and `<note>`, each with a
markdown body, and three labelled code fences: ` ```run `, ` ```output `
and ` ```file <path> `. A screenshot is
`<showcase type="image" src="images/<file>.webp" frame="WxH" alt="…" caption="…" />`,
Supramental Gold's `image-figure`, with the file in `docs/images/`.

Build only in CI or in a container that does not mount your home
directory: a local build runs every dependency as you, and Paircaster's app
keys live under your home directory.

```sh
mkdir -p content && cp -R ../paircaster/docs/. content/
docker run --rm -v "$PWD":/site -w /site oven/bun:1.3.14 \
  sh -c 'bun install --frozen-lockfile && bun run build'
```
