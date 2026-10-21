import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ejs from 'ejs';
import ejsPlugin from '@11ty/eleventy-plugin-ejs';
import sgPlugin, { createCustomElementRenderer } from '@cyberchitta/supramental-gold/eleventy';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const sgEleventy = path.join(
  path.dirname(fileURLToPath(import.meta.resolve('@cyberchitta/supramental-gold/package.json'))),
  'eleventy',
);
const includesDir = path.join(__dirname, '_includes');
const site = JSON.parse(fs.readFileSync(path.join(__dirname, '_data', 'site.json'), 'utf8'));

// Boxes the page uses to say what was not run and what Selfcast cannot do.
// Authored as <not-run>, <limit> and <note>, each with a markdown body.
// All three share the neutral alert: alert-warning's yellow leaves inline
// code unreadable, so the label carries the meaning.
const CALLOUTS = {
  'not-run': { label: 'Not yet run' },
  limit: { label: 'Limit' },
  note: { label: '' },
};

// Fenced blocks labelled by their info string: ```run, ```output, ```file <path>.
const FENCE_LABELS = { run: 'run', output: 'output' };
const labelledCode = ejs.compile(
  fs.readFileSync(path.join(includesDir, 'elements', 'labelled-code.ejs'), 'utf8'),
);

function fenceLabel(info) {
  const [kind, ...rest] = info.trim().split(/\s+/);
  if (kind === 'file') return rest.join(' ');
  return FENCE_LABELS[kind] ?? null;
}

function labelFences(md) {
  const fence = md.renderer.rules.fence;
  md.renderer.rules.fence = (tokens, idx, options, env, self) => {
    const label = fenceLabel(tokens[idx].info);
    if (label === null) return fence(tokens, idx, options, env, self);
    tokens[idx].info = '';
    return labelledCode({ label, pre: fence(tokens, idx, options, env, self) });
  };
}

export default function (eleventyConfig) {
  eleventyConfig.addPlugin(ejsPlugin, { views: [includesDir, sgEleventy] });
  eleventyConfig.addPlugin(sgPlugin, { internalDomains: [new URL(site.url).hostname] });
  eleventyConfig.amendLibrary('md', labelFences);
  eleventyConfig.addGlobalData('layout', 'layouts/page.ejs');

  const renderCallouts = createCustomElementRenderer(
    Object.fromEntries(Object.keys(CALLOUTS).map((tag) => [tag, 'elements/callout.ejs'])),
    includesDir,
    { data: { site } },
    Object.fromEntries(
      Object.entries(CALLOUTS).map(([tag, style]) => [tag, (body) => ({ body, ...style })]),
    ),
  );
  eleventyConfig.addTransform('renderCallouts', async function (content) {
    return this.page.outputPath?.endsWith('.html') ? renderCallouts(content) : content;
  });

  // The words come from main's docs/, copied into content/ by the workflow,
  // which is why content/ is gitignored here and the gitignore is not used.
  eleventyConfig.setUseGitIgnore(false);
  // upstream-fixes.md is a working record kept in the repository, not a page.
  eleventyConfig.ignores.add('content/upstream-fixes.md');

  return {
    dir: { input: 'content', output: '_site', includes: '../_includes', data: '../_data' },
    markdownTemplateEngine: 'ejs',
    htmlTemplateEngine: 'ejs',
    templateFormats: ['md'],
  };
}
