// Builds the viewer into `dist/`: `index.html` with the style inline, and
// `viewer.js` holding the Rust reads compiled to WebAssembly (crates/wasm),
// its glue and the page's script. Shala's page policy refuses inline
// scripts. Shala serves `dist/` as the project's App tab and claims the
// `paircaster` scheme for it from the meta tag.
//
// Needs the wasm32-unknown-unknown target and wasm-bindgen-cli at the
// version in Cargo.lock.

import { $ } from "bun";

const root = `${import.meta.dir}/..`;
await $`cargo build -q -p paircaster-wasm --target wasm32-unknown-unknown --profile wasm`.cwd(root);
await $`wasm-bindgen --target web --out-dir viewer/wasm target/wasm32-unknown-unknown/wasm/paircaster_wasm.wasm`.cwd(root);
const wasm = await Bun.file(`${import.meta.dir}/wasm/paircaster_wasm_bg.wasm`).arrayBuffer();
await Bun.write(
  `${import.meta.dir}/wasm/module.ts`,
  `export default "${Buffer.from(wasm).toString("base64")}";\n`,
);

const built = await Bun.build({
  entrypoints: [`${import.meta.dir}/src/main.tsx`],
  target: "browser",
  minify: true,
});
if (!built.success) {
  console.error(built.logs.join("\n"));
  process.exit(1);
}
const script = await built.outputs[0].text();
const style = await Bun.file(`${import.meta.dir}/src/viewer.css`).text();

const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="shala-scheme" content="paircaster">
<title>Paircaster</title>
<style>${style}</style>
</head>
<body>
<div id="app"></div>
<script type="module" src="viewer.js"></script>
</body>
</html>
`;
await Bun.write(`${import.meta.dir}/dist/viewer.js`, script);
await Bun.write(`${import.meta.dir}/dist/index.html`, html);
console.log(`dist/: index.html ${html.length} bytes, viewer.js ${script.length} (wasm ${wasm.byteLength})`);
