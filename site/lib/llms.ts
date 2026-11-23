import path from 'node:path';
import { Marked } from 'marked';
import { categories } from '@/catalog';
import { apiHref, apiItems, CLASS_ORDER } from './apiref';
import { docGroups, docSource, type DocPage } from './docs';
import { describe, SITE_NAME, SITE_URL, SITE_VERSION } from './seo';

const markdown = new Marked({ gfm: true });
const absolute = (href: string) => new URL(href, SITE_URL).href;
const guides = () => docGroups().map((group) => ({
  ...group, pages: group.pages.filter((page) => !page.file.startsWith('api:')),
})).filter((group) => group.pages.length);
const line = (title: string, href: string, note: string) =>
  `- [${title}](${absolute(href)})${note ? `: ${note}` : ''}`;

/** A short, single-line summary taken from the source, without Markdown links;
 * empty when the first paragraph only introduces a list or table (ends with a colon). */
function summary(source: string, fallback: string): string {
  const text = describe(source, fallback);
  const first = text.match(/^.*?[.!?](?=\s|$)/)?.[0] ?? text;
  return first.endsWith(':') ? '' : first;
}

/** Keep code examples intact; rewrite prose links against the chapter's public URL. */
export function guideMarkdown(source: string, page: DocPage, pages: DocPage[]): string {
  const chapterLinks = new Map(pages.map((p) => [p.file, p.href]));
  const resolve = (href: string): string | null => {
    if (/^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(href)) {
      const url = new URL(href, SITE_URL);
      return url.origin === SITE_URL ? url.href : null;
    }
    if (href.startsWith('#')) return absolute(page.href) + href;
    if (href.startsWith('/')) return absolute(href);
    const boundary = href.search(/[?#]/);
    const file = boundary < 0 ? href : href.slice(0, boundary);
    const suffix = boundary < 0 ? '' : href.slice(boundary);
    const target = chapterLinks.get(path.posix.normalize(path.posix.join(path.posix.dirname(page.file), file)));
    if (!target) throw new Error(`Unknown guide link: ${page.file}: ${href}`);
    return absolute(target) + suffix;
  };
  // mdBook directives are presentation instructions, not standalone documentation.
  const sourceText = source.replace(/\{\{#[\s\S]*?\}\}/g, '');
  return markdown.lexer(sourceText).map((token) => {
    if (token.type === 'code') return token.raw;
    return token.raw.replace(/(`+)[\s\S]*?\1|(!?)\[([^\]]*)\]\(([^\s)]+)(?:\s+"[^"]*")?\)/g,
      (raw, code: string | undefined, image: string, label: string, href: string) => {
        if (code) return raw;
        if (image) return label;
        const url = resolve(href);
        // External references retain their labels; this text edition links only to this site.
        return url ? `[${label}](${url})` : label;
      });
  }).join('');
}

/** Compact entry point; the complete API inventory remains at the linked reference index. */
export function llmsIndex(): string {
  const intro = docSource('introduction.md').split(/\n\s*\n/)[1].replace(/\s+/g, ' ').trim();
  const install = (file: string, command: string) => {
    const match = docSource(file).split('\n').find((line) => line.startsWith(command));
    if (!match) throw new Error(`Missing installation command in ${file}`);
    return match.split(/\s+#/)[0].trim();
  };
  const sections = guides().map((group) => `## ${group.title}\n\n${group.pages.map((page) =>
    line(page.title, page.href, summary(docSource(page.file), page.title)),
  ).join('\n')}`);
  const items = apiItems();
  const classes = CLASS_ORDER.flatMap((name) => items.filter((item) => item.name === name));
  sections.push(`## JavaScript API reference\n\n${[
    line('All API items', '/docs/api', 'Complete generated class, function, interface and type reference.'),
    ...classes.map((item) => line(item.name, apiHref(item.name), summary(item.summary, `${item.kind} reference.`))),
  ].join('\n')}`);
  sections.push(`## Example categories\n\n${categories.map((category) =>
    line(category.title, `/charts/${category.id}/${category.demos[0].id}`, summary(category.blurb, category.title)),
  ).join('\n')}`);
  return [
    `# ${SITE_NAME}`,
    `> ${summary(intro, SITE_NAME)}`,
    intro,
    `Version: ${SITE_VERSION}. License: Apache-2.0.`,
    `JavaScript / TypeScript: \`${install('web.md', 'npm install ')}\`. Python: \`${install('python.md', 'pip install ')}\`.`,
    `## Complete guide\n\n${line('Full text', '/llms-full.txt', 'Every English guide chapter in reading order, followed by the example catalog.')}`,
    ...sections,
  ].join('\n\n') + '\n';
}

/** The English book and example catalog, generated from the same sources as the website. */
export function llmsFull(): string {
  const pages = guides().flatMap((group) => group.pages);
  const chapters = pages.map((page) => {
    const content = guideMarkdown(docSource(page.file), page, pages);
    return `Source: ${absolute(page.href)}\n\n${content.trim()}`;
  });
  const examples = categories.map((category) => `## ${category.title}\n\n${category.demos.map((demo) =>
    line(demo.title, `/charts/${category.id}/${demo.id}`, summary(demo.about, demo.title)),
  ).join('\n')}`);
  return [
    [
      `# ${SITE_NAME} — complete guide`,
      `Version: ${SITE_VERSION}. License: Apache-2.0.`,
      `Index: ${absolute('/llms.txt')}`,
    ].join('\n\n'),
    ...chapters,
    '# Example catalog',
    ...examples,
  ].join('\n\n---\n\n') + '\n';
}
