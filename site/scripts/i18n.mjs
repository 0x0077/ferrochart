// Translations of the site (i18n/): the catalog text and the user guide.
//   node scripts/i18n.mjs extract   writes i18n/catalog/en.json from catalog/*.ts
//   node scripts/i18n.mjs stamp     records the English source of every translation present
//   node scripts/i18n.mjs check     lists missing and outdated translations (exit 1 if any)
// A translation records a hash of the English it was made from ("src" in the
// catalog files, a first-line comment in the guide), so a change of the English
// shows up here until the translation is updated and stamped again.
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createJiti } from 'jiti';

const SITE = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const I18N = path.join(SITE, 'i18n');
const LOCALES = ['zh', 'zh-tw', 'ja', 'ko'];
const VENDORED = path.join(SITE, 'vendor', 'docs');
const BOOK = existsSync(VENDORED) ? VENDORED : path.join(SITE, '..', 'docs', 'book', 'src');
const hash = (s) => createHash('sha256').update(s).digest('hex').slice(0, 10);
const SRC_LINE = /^<!-- source: ([0-9a-f]+) -->\n/;

async function english() {
  const jiti = createJiti(import.meta.url, { alias: { '@': SITE } });
  const { categories } = await jiti.import(path.join(SITE, 'catalog', 'index.ts'));
  const out = { categories: {}, groups: {}, demos: {} };
  for (const c of categories) {
    out.categories[c.id] = { title: c.title, blurb: c.blurb };
    for (const d of c.demos) {
      if (out.demos[d.id]) throw new Error(`duplicate demo id ${d.id}`);
      out.demos[d.id] = { title: d.title, about: d.about };
      if (d.group) out.groups[d.group] = d.group;
    }
  }
  return out;
}

const json = (f) => JSON.parse(readFileSync(f, 'utf8'));
const write = (f, v) => writeFileSync(f, `${JSON.stringify(v, null, 2)}\n`);
const chapters = () => readdirSync(BOOK).filter((f) => f.endsWith('.md'));

const cmd = process.argv[2];
const en = await english();
if (cmd === 'extract') {
  write(path.join(I18N, 'catalog', 'en.json'), en);
  console.log(`${Object.keys(en.demos).length} demos, ${Object.keys(en.categories).length} categories, ${Object.keys(en.groups).length} groups`);
} else if (cmd === 'stamp' || cmd === 'check') {
  const problems = [];
  for (const l of LOCALES) {
    const file = path.join(I18N, 'catalog', `${l}.json`);
    if (!existsSync(file)) { problems.push(`${l}: no catalog translation`); continue; }
    const t = json(file);
    for (const kind of ['categories', 'demos']) {
      for (const [id, e] of Object.entries(en[kind])) {
        const x = t[kind]?.[id];
        const h = hash(JSON.stringify(e));
        if (!x) problems.push(`${l}: ${kind}/${id} missing`);
        else if (cmd === 'stamp') x.src ??= h;
        else if (x.src !== h) problems.push(`${l}: ${kind}/${id} outdated`);
      }
    }
    for (const g of Object.keys(en.groups)) if (!t.groups?.[g]) problems.push(`${l}: group "${g}" missing`);
    if (cmd === 'stamp') write(file, t);
    for (const f of chapters()) {
      const tf = path.join(I18N, 'docs', l, f);
      if (!existsSync(tf)) { problems.push(`${l}: docs/${f} missing`); continue; }
      const h = hash(readFileSync(path.join(BOOK, f), 'utf8'));
      const text = readFileSync(tf, 'utf8');
      const m = text.match(SRC_LINE);
      if (cmd === 'stamp' && !m) writeFileSync(tf, `<!-- source: ${h} -->\n${text}`);
      else if (cmd === 'check' && m?.[1] !== h) problems.push(`${l}: docs/${f} outdated`);
    }
  }
  for (const p of problems) console.log(p);
  console.log(`${cmd}: ${problems.length} problems`);
  if (cmd === 'check' && problems.length) process.exit(1);
} else {
  console.error('usage: i18n.mjs extract|stamp|check');
  process.exit(2);
}
