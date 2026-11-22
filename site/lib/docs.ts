// The documentation pages: the user guide in docs/book/src (also an mdBook),
// read and rendered at build time. SUMMARY.md gives the order and the
// groups; introduction.md is /docs, every other chapter /docs/<name>.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { Marked, type Tokens } from 'marked';
import { highlight } from './highlight';
import { CLASS_ORDER, apiIndexHtml, apiPage } from './apiref';

const ROOT = path.join(process.cwd(), '..', 'docs', 'book', 'src');
/** Source files linked from the book, shown on GitHub. */
const REPO = 'https://github.com/0x0077/ferrochart/blob/main/';

export type DocPage = { slug: string; title: string; file: string };
export type DocGroup = { title: string; pages: DocPage[] };
export type Heading = { id: string; text: string; depth: 2 | 3 };

const slugOf = (file: string) => (file === 'introduction.md' ? '' : file.replace(/\.md$/, ''));
export const hrefOf = (slug: string) => (slug ? `/docs/${slug}` : '/docs');

/** The groups of SUMMARY.md: a `# Title` line starts a group, `- [Title](file.md)` is a page. */
export function docGroups(): DocGroup[] {
  const groups: DocGroup[] = [{ title: 'Overview', pages: [] }];
  for (const line of readFileSync(path.join(ROOT, 'SUMMARY.md'), 'utf8').split('\n')) {
    const group = line.match(/^# (.+)/);
    if (group && group[1] !== 'Summary') groups.push({ title: group[1], pages: [] });
    const page = line.match(/\[(.+?)\]\((.+?\.md)\)/);
    if (page) groups[groups.length - 1].pages.push({ title: page[1], file: page[2], slug: slugOf(page[2]) });
  }
  groups.push({
    title: 'JavaScript API',
    pages: [
      { title: 'All classes and types', file: 'api:', slug: 'api' },
      ...CLASS_ORDER.map((n) => ({ title: n, file: `api:${n}`, slug: `api/${n}` })),
    ],
  });
  return groups.filter((g) => g.pages.length);
}

/** A page's content: a book chapter, the API index or an API item. */
export function docContent(page: DocPage): { html: string; title: string; headings: Heading[] } | null {
  if (!page.file.startsWith('api:')) return renderDoc(page.file);
  const name = page.file.slice(4);
  if (!name) return { ...apiIndexHtml(), title: 'JavaScript API reference' };
  const p = apiPage(name);
  return p ? { html: p.html, title: p.name, headings: p.headings } : null;
}

export const docPages = () => docGroups().flatMap((g) => g.pages);

/** Fence languages → highlight.js languages; others are escaped only. */
const LANG: Record<string, string> = {
  js: 'js', javascript: 'js', ts: 'js', typescript: 'js', tsx: 'js', jsx: 'js', vue: 'js',
  rust: 'rust', python: 'python', py: 'python', c: 'c', cpp: 'c', csharp: 'c', cs: 'c',
  bash: 'sh', sh: 'sh', shell: 'sh', console: 'sh',
};
const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const plain = (html: string) => html.replace(/<[^>]+>/g, '').replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'");
const slugify = (text: string) => text.toLowerCase().replace(/[`'"]/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');

/** Renders chapter `file`: its HTML, title and h2/h3 outline. */
export function renderDoc(file: string): { html: string; title: string; headings: Heading[] } {
  const src = readFileSync(path.join(ROOT, file), 'utf8');
  const headings: Heading[] = [];
  const used = new Map<string, number>();
  let title = '';
  const md = new Marked({ gfm: true });
  md.use({
    renderer: {
      heading(this: { parser: { parseInline(t: Tokens.Generic[]): string } }, { tokens, depth }: Tokens.Heading) {
        const inner = this.parser.parseInline(tokens);
        const text = plain(inner);
        if (depth === 1) {
          title ||= text;
          return `<h1>${inner}</h1>\n`;
        }
        const base = slugify(text) || 'section';
        const n = used.get(base) ?? 0;
        used.set(base, n + 1);
        const id = n ? `${base}-${n}` : base;
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
        const chapter = href.match(/^([\w-]+)\.md(#.*)?$/);
        if (chapter) url = hrefOf(slugOf(`${chapter[1]}.md`)) + (chapter[2] ?? '');
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
  return { html, title, headings };
}
