// The documentation pages: the user guide in docs/book/src (also an mdBook),
// read and rendered at build time. SUMMARY.md gives the order and the
// groups; introduction.md is /docs, every other chapter /docs/<name>.
// Translations are in i18n/docs/<locale>/ (the same files, /zh/docs/...); a
// chapter without one falls back to English. Heading anchors always come
// from the English chapter, so links to a section work in every language.
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { Marked, type Tokens } from 'marked';
import { highlight } from './highlight';
import { CLASS_ORDER, apiIndexHtml, apiPage } from './apiref';
import { localePath, type Locale } from './i18n';
import { messages } from './messages';

// In the site's own repository the guide is a copy in vendor/docs.
const VENDORED = path.join(process.cwd(), 'vendor', 'docs');
const ROOT = existsSync(VENDORED) ? VENDORED : path.join(process.cwd(), '..', 'docs', 'book', 'src');
/** Source files linked from the book, shown on GitHub. */
const REPO = 'https://github.com/0x0077/ferrochart/blob/main/';

export type DocPage = { slug: string; title: string; file: string; href: string };
export type DocGroup = { title: string; pages: DocPage[] };
export type Heading = { id: string; text: string; depth: 2 | 3 };

const slugOf = (file: string) => (file === 'introduction.md' ? '' : file.replace(/\.md$/, ''));
/** The address of a chapter in `locale` (the API reference is English only). */
export const hrefOf = (slug: string, locale: Locale = 'en') => localePath(locale, slug ? `/docs/${slug}` : '/docs');

/** The translation of chapter `file` in `locale`, if there is one. */
function translated(file: string, locale: Locale): string | null {
  if (locale === 'en') return null;
  const f = path.join(process.cwd(), 'i18n', 'docs', locale, file);
  return existsSync(f) ? readFileSync(f, 'utf8').replace(/^<!-- source: [0-9a-f]+ -->\n/, '') : null;
}

/** The groups of SUMMARY.md: a `# Title` line starts a group, `- [Title](file.md)` is a page. */
export function docGroups(locale: Locale = 'en'): DocGroup[] {
  const t = messages(locale).docs;
  const groups: DocGroup[] = [{ title: t.overview, pages: [] }];
  const summary = translated('SUMMARY.md', locale) ?? readFileSync(path.join(ROOT, 'SUMMARY.md'), 'utf8');
  for (const line of summary.split('\n')) {
    const group = line.match(/^# (.+)/);
    if (group && group[1] !== 'Summary') groups.push({ title: group[1], pages: [] });
    const page = line.match(/\[(.+?)\]\((.+?\.md)\)/);
    if (page) {
      const slug = slugOf(page[2]);
      groups[groups.length - 1].pages.push({ title: page[1], file: page[2], slug, href: hrefOf(slug, locale) });
    }
  }
  groups.push({
    title: t.jsApi,
    pages: [
      { title: t.allClasses, file: 'api:', slug: 'api', href: '/docs/api' },
      ...CLASS_ORDER.map((n) => ({ title: n, file: `api:${n}`, slug: `api/${n}`, href: `/docs/api/${n}` })),
    ],
  });
  return groups.filter((g) => g.pages.length);
}

/** A page's content: a book chapter, the API index or an API item. */
export function docContent(page: DocPage, locale: Locale = 'en'): { html: string; title: string; headings: Heading[]; fallback?: boolean } | null {
  if (!page.file.startsWith('api:')) return renderDoc(page.file, locale);
  const name = page.file.slice(4);
  if (!name) return { ...apiIndexHtml(), title: 'JavaScript API reference' };
  const p = apiPage(name);
  return p ? { html: p.html, title: p.name, headings: p.headings } : null;
}

export const docPages = (locale: Locale = 'en') => docGroups(locale).flatMap((g) => g.pages);

/** Fence languages → highlight.js languages; others are escaped only. */
const LANG: Record<string, string> = {
  js: 'js', javascript: 'js', ts: 'js', typescript: 'js', tsx: 'js', jsx: 'js', vue: 'js',
  rust: 'rust', python: 'python', py: 'python', c: 'c', cpp: 'c', csharp: 'c', cs: 'c',
  bash: 'sh', sh: 'sh', shell: 'sh', console: 'sh',
};
const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const plain = (html: string) => html.replace(/<[^>]+>/g, '').replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'");
const slugify = (text: string) => text.toLowerCase().replace(/[`'"]/g, '').replace(/[^\p{L}\p{N}]+/gu, '-').replace(/^-|-$/g, '');
const englishSlug = (text: string) => text.toLowerCase().replace(/[`'"]/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');

/** The anchors of chapter `file`'s headings in English, in order (`null` for h1). */
function englishAnchors(file: string): (string | null)[] {
  const used = new Map<string, number>();
  const out: (string | null)[] = [];
  const md = new Marked({ gfm: true });
  for (const tok of md.lexer(readFileSync(path.join(ROOT, file), 'utf8'))) {
    if (tok.type !== 'heading') continue;
    if (tok.depth === 1) { out.push(null); continue; }
    const text = plain(md.parseInline(tok.text, { async: false }) as string);
    const base = englishSlug(text) || 'section';
    const n = used.get(base) ?? 0;
    used.set(base, n + 1);
    out.push(n ? `${base}-${n}` : base);
  }
  return out;
}

/** The Markdown source of chapter `file` in `locale` (English if it is not translated). */
export function docSource(file: string, locale: Locale = 'en'): string {
  return translated(file, locale) ?? readFileSync(path.join(ROOT, file), 'utf8');
}

/** Renders chapter `file` in `locale`: its HTML, title and h2/h3 outline; `fallback` if shown in English. */
export function renderDoc(file: string, locale: Locale = 'en'): { html: string; title: string; headings: Heading[]; fallback: boolean } {
  const local = translated(file, locale);
  const src = local ?? readFileSync(path.join(ROOT, file), 'utf8');
  const anchors = local ? englishAnchors(file) : null;
  let nth = 0;
  const headings: Heading[] = [];
  const used = new Map<string, number>();
  let title = '';
  const md = new Marked({ gfm: true });
  md.use({
    renderer: {
      heading(this: { parser: { parseInline(t: Tokens.Generic[]): string } }, { tokens, depth }: Tokens.Heading) {
        const inner = this.parser.parseInline(tokens);
        const text = plain(inner);
        const anchor = anchors?.[nth++];
        if (depth === 1) {
          title ||= text;
          return `<h1>${inner}</h1>\n`;
        }
        const base = slugify(text) || 'section';
        const n = used.get(base) ?? 0;
        used.set(base, n + 1);
        const id = anchor ?? (n ? `${base}-${n}` : base);
        if (depth === 2 || depth === 3) headings.push({ id, text, depth });
        return `<h${depth} id="${id}"><a class="anchor" href="#${id}" aria-hidden="true" tabindex="-1">#</a>${inner}</h${depth}>\n`;
      },
      code({ text, lang }: Tokens.Code) {
        const name = (lang ?? '').split(/[,\s]/)[0].toLowerCase();
        const hl = LANG[name];
        const body = hl ? highlight(text, hl) : esc(text);
        const label = name && name !== 'text' ? `<span class="lang">${esc(name)}</span>` : '';
        return `<div class="code">${label}<pre><code>${body}</code></pre></div>\n`;
      },
      link(this: { parser: { parseInline(t: Tokens.Generic[]): string } }, { href, title: t, tokens }: Tokens.Link) {
        const inner = this.parser.parseInline(tokens);
        // Links to this site (written as absolute URLs so the mdBook works too) stay on it.
        let url = href.replace(/^https:\/\/ferrochart\.com(?=\/)/, '');
        if (url.startsWith('/')) url = localePath(locale, url);
        const chapter = href.match(/^([\w-]+)\.md(#.*)?$/);
        if (chapter) url = hrefOf(slugOf(`${chapter[1]}.md`), locale) + (chapter[2] ?? '');
        else if (/^(\.\.\/)+/.test(href)) url = REPO + href.replace(/^(\.\.\/)+/, '');
        const external = /^https?:/.test(url);
        return `<a href="${esc(url)}"${t ? ` title="${esc(t)}"` : ''}${external ? ' target="_blank" rel="noopener"' : ''}>${inner}</a>`;
      },
      table(this: { parser: { parseInline(t: Tokens.Generic[]): string } }, token: Tokens.Table) {
        const cell = (c: Tokens.TableCell, tag: string) =>
          `<${tag}${c.align ? ` style="text-align:${c.align}"` : ''}>${this.parser.parseInline(c.tokens)}</${tag}>`;
        const head = `<tr>${token.header.map((c) => cell(c, 'th')).join('')}</tr>`;
        const rows = token.rows.map((r) => `<tr>${r.map((c) => cell(c, 'td')).join('')}</tr>`).join('');
        return `<div class="table"><table><thead>${head}</thead><tbody>${rows}</tbody></table></div>\n`;
      },
    },
  });
  const html = md.parse(src, { async: false }) as string;
  return { html, title, headings, fallback: locale !== 'en' && !local };
}
