// Checks translated guide chapters against the English: the same headings
// (levels, in order), identical fenced code blocks, the same link targets and
// the same number of table rows.
//   node scripts/i18n-doc-verify.mjs <locale> [chapter.md ...]   (all chapters by default)
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SITE = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const VENDORED = path.join(SITE, 'vendor', 'docs');
const BOOK = existsSync(VENDORED) ? VENDORED : path.join(SITE, '..', 'docs', 'book', 'src');
const [locale, ...files] = process.argv.slice(2);
if (!locale) { console.error('usage: i18n-doc-verify.mjs <locale> [chapter.md ...]'); process.exit(2); }

/** Headings, code blocks, link targets and table rows of a chapter. */
function shape(md) {
  md = md.replace(/^<!-- source: [0-9a-f]+ -->\n/, '');
  const code = [];
  const prose = md.replace(/^(```|~~~)[^\n]*\n[\s\S]*?^\1[ \t]*$/gm, (m) => { code.push(m.trim()); return '\n'; });
  const headings = [...prose.matchAll(/^(#{1,6}) /gm)].map((m) => m[1].length).join(',');
  const links = [...prose.matchAll(/\]\(([^)\s]+)/g)].map((m) => m[1]).sort();
  const rows = (prose.match(/^\s*\|.*\|\s*$/gm) ?? []).length;
  return { headings, code, links, rows };
}

const list = files.length ? files : readdirSync(BOOK).filter((f) => f.endsWith('.md'));
let bad = 0;
for (const f of list) {
  const tf = path.join(SITE, 'i18n', 'docs', locale, f);
  if (!existsSync(tf)) { console.log(`${f}: missing`); bad++; continue; }
  const a = shape(readFileSync(path.join(BOOK, f), 'utf8'));
  const b = shape(readFileSync(tf, 'utf8'));
  const errs = [];
  if (a.headings !== b.headings) errs.push(`headings differ: en [${a.headings}] vs [${b.headings}]`);
  if (a.code.length !== b.code.length) errs.push(`${a.code.length} code blocks in English, ${b.code.length} here`);
  else a.code.forEach((c, i) => { if (c !== b.code[i]) errs.push(`code block ${i + 1} differs`); });
  if (a.links.join(' ') !== b.links.join(' ')) errs.push(`link targets differ: missing [${a.links.filter((l) => !b.links.includes(l)).join(' ')}] extra [${b.links.filter((l) => !a.links.includes(l)).join(' ')}]`);
  if (a.rows !== b.rows) errs.push(`table rows: ${a.rows} in English, ${b.rows} here`);
  if (errs.length) { bad++; console.log(`${f}:\n  ${errs.join('\n  ')}`); }
}
console.log(bad ? `${bad} of ${list.length} chapters need fixing` : `ok: ${list.length} chapters`);
process.exit(bad ? 1 : 0);
