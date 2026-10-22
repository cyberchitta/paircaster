// Builds the viewer into one HTML file with everything inline: the Rust
// reads compiled to WebAssembly (crates/wasm), its glue, the page's script
// and style. `selfcast mcp` embeds the result, so it is committed.
//
// Needs the wasm32-unknown-unknown target and wasm-bindgen-cli at the
// version in Cargo.lock.

import { $ } from "bun";

const root = `${import.meta.dir}/..`;
await $`cargo build -q -p selfcast-wasm --target wasm32-unknown-unknown --profile wasm`.cwd(root);
await $`wasm-bindgen --target web --out-dir viewer/wasm target/wasm32-unknown-unknown/wasm/selfcast_wasm.wasm`.cwd(root);
const wasm = await Bun.file(`${import.meta.dir}/wasm/selfcast_wasm_bg.wasm`).arrayBuffer();
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
const script = (await built.outputs[0].text()).replaceAll("</script", "<\\/script");
const style = await Bun.file(`${import.meta.dir}/src/viewer.css`).text();

const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Selfcast</title>
<style>${style}</style>
</head>
<body>
<div id="app"></div>
<script type="module">${script}</script>
</body>
</html>
`;
await Bun.write(`${import.meta.dir}/dist/viewer.html`, html);
console.log(`dist/viewer.html: ${html.length} bytes (wasm ${wasm.byteLength})`);
