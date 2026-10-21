import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(readFileSync(join(__dirname, '..', 'package.json'), 'utf8'));
const dep = pkg.dependencies['@cyberchitta/supramental-gold'];
const version = dep.split('#').pop();
const base = `https://cdn.jsdelivr.net/gh/cyberchitta/supramental-gold@${version}`;

export default {
  version,
  base,
  cssBundleUrl: `${base}/dist/styles.css`,
  logoSvgUrl: `${base}/assets/cc-260928.svg`,
  logoPngUrl: `${base}/assets/cc-260928.png`,
};
