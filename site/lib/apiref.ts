// The JavaScript API reference, generated at build time from the published
// TypeScript declarations (crates/ferrochart-wasm/pkg/ferrochart_wasm.d.ts,
// made by wasm-bindgen from the Rust doc comments). Every class, interface,
// type and function gets a page under /docs/api/<Name>; type names in
// signatures link to their pages.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { Marked } from 'marked';
import ts from 'typescript';
import { highlight } from './highlight';
import type { Heading } from './docs';

const DTS = path.join(process.cwd(), '..', 'crates', 'ferrochart-wasm', 'pkg', 'ferrochart_wasm.d.ts');

/** wasm-bindgen's loader internals: not part of the documented API. */
const HIDDEN = new Set(['InitInput', 'InitOutput', 'SyncInitInput', '__wbg_init']);

export type ApiKind = 'class' | 'interface' | 'type' | 'function';
export type ApiItem = { name: string; kind: ApiKind; summary: string };
export type ApiPage = ApiItem & { html: string; headings: Heading[] };

/** The classes in reading order (the sidebar lists them). */
export const CLASS_ORDER = ['FerroChart', 'Series', 'Indicator', 'PriceLine', 'Drawings', 'SyncGroup', 'FerroPlot', 'Mark', 'Axis', 'Rule', 'Label'];

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const md = new Marked({ gfm: true });
const printer = ts.createPrinter({ removeComments: true });

let cache: { items: Map<string, { item: ApiItem; node: ts.Statement }>; merged: Map<string, ts.InterfaceDeclaration[]>; source: ts.SourceFile } | null = null;

function load() {
  if (cache) return cache;
  const text = readFileSync(DTS, 'utf8');
  const source = ts.createSourceFile('ferrochart_wasm.d.ts', text, ts.ScriptTarget.Latest, true);
  const items = new Map<string, { item: ApiItem; node: ts.Statement }>();
  // Interfaces merged into a class of the same name (typed `on`/`off`): their members belong on the class page.
  const merged = new Map<string, ts.InterfaceDeclaration[]>();
  const classes = new Set(source.statements.filter(ts.isClassDeclaration).map((c) => c.name?.text));
  for (const st of source.statements) {
    if (ts.isInterfaceDeclaration(st) && classes.has(st.name.text)) {
      merged.set(st.name.text, [...(merged.get(st.name.text) ?? []), st]);
      continue;
    }
    let name: string | undefined;
    let kind: ApiKind | undefined;
    if (ts.isClassDeclaration(st) && st.name) [name, kind] = [st.name.text, 'class'];
    else if (ts.isInterfaceDeclaration(st)) [name, kind] = [st.name.text, 'interface'];
    else if (ts.isTypeAliasDeclaration(st)) [name, kind] = [st.name.text, 'type'];
    else if (ts.isFunctionDeclaration(st) && st.name) [name, kind] = [st.name.text, 'function'];
    if (!name || !kind || HIDDEN.has(name) || name === 'initSync') continue;
    items.set(name, { item: { name, kind, summary: '' }, node: st });
  }
  cache = { items, merged, source };
  // Summaries link type names, which needs the full list first.
  for (const { item, node } of items.values()) item.summary = firstSentence(docOf(node));
  return cache;
}

export const apiItems = (): ApiItem[] => [...load().items.values()].map((x) => x.item);
export const apiHref = (name: string) => `/docs/api/${name}`;

/** The doc comment text of a node (without tags). */
function docOf(node: ts.Node): string {
  const docs = (node as { jsDoc?: ts.JSDoc[] }).jsDoc ?? [];
  return docs.map((d) => ts.getTextOfJSDocComment(d.comment) ?? '').join('\n\n').trim();
}

/** Described `@param` and `@returns` tags (wasm-bindgen also emits empty, type-only ones). */
function tagsOf(node: ts.Node): { params: [string, string][]; returns: string } {
  const params: [string, string][] = [];
  let returns = '';
  for (const d of (node as { jsDoc?: ts.JSDoc[] }).jsDoc ?? []) {
    for (const t of d.tags ?? []) {
      const text = (ts.getTextOfJSDocComment(t.comment) ?? '').replace(/^-\s*/, '').trim();
      if (!text) continue;
      if (ts.isJSDocParameterTag(t)) params.push([t.name.getText(), text]);
      else if (ts.isJSDocReturnTag(t)) returns = text;
    }
  }
  return { params, returns };
}

function firstSentence(doc: string): string {
  const p = doc.split(/\n\s*\n/)[0].replace(/\s+/g, ' ').trim();
  const m = p.match(/^(.+?[.:])(\s|$)/);
  return clean((m ? m[1] : p).replace(/:$/, '.'));
}

/** Doc text for readers of the site: rustdoc links become links, internal section numbers go. */
function clean(doc: string): string {
  const names = load().items;
  return doc
    .replace(/\s*\(§[\d.]+\)/g, '')
    .replace(/<https:\/\/ferrochart\.com(\/[^>]*)>/g, '[$1]($1)')
    .replace(/\]\(https:\/\/ferrochart\.com\//g, '](/')
    .replace(/docs\/indicators\.md/g, '[Indicators](/docs/indicators)')
    .replace(/docs\/transforms\.md/g, '[Series types](/docs/series)')
    .replace(/\[`([\w.]+)`\](?!\()/g, (_, n: string) => (names.has(n.split('.')[0]) ? `[\`${n}\`](${apiHref(n.split('.')[0])})` : `\`${n}\``));
}

const renderMd = (doc: string) => (doc ? (md.parse(clean(doc), { async: false }) as string) : '');

/** Highlighted TypeScript with links on the API's type names. */
function code(text: string, self?: string): string {
  const names = load().items;
  // Type names outside strings and comments become links.
  const html = highlight(text, 'js')
    .split(/(<span class="tok-[sc]">[^<]*<\/span>)/)
    .map((part) => (part.startsWith('<span class="tok-s') || part.startsWith('<span class="tok-c')
      ? part
      : part.replace(/(?<![\w.$#-])([A-Z]\w+)(?![\w-])/g, (m, n: string) => (names.has(n) && n !== self ? `<a href="${apiHref(n)}">${n}</a>` : m))))
    .join('');
  return `<div class="code api-sig"><pre><code>${html}</code></pre></div>`;
}

/** A declaration without its comments, for display. */
const printed = (node: ts.Node) => printer.printNode(ts.EmitHint.Unspecified, node, load().source).trim();

function memberName(m: ts.ClassElement | ts.TypeElement): string {
  if (ts.isConstructorDeclaration(m)) return 'constructor';
  const n = m.name;
  if (!n) return '';
  return ts.isComputedPropertyName(n) ? n.getText() : n.getText().replace(/^["']|["']$/g, '');
}

function memberSection(m: ts.ClassElement | ts.TypeElement, self: string, headings: Heading[], used: Set<string>, prefix = ''): string {
  const name = memberName(m);
  const label = prefix + name;
  let id = label.replace(/[^\w-]+/g, '-').toLowerCase();
  while (used.has(id)) id += '-';
  used.add(id);
  headings.push({ id, text: label, depth: 3 });
  const { params, returns } = tagsOf(m);
  let out = `<h3 id="${id}"><a class="anchor" href="#${id}" aria-hidden="true" tabindex="-1">#</a><code>${esc(label)}</code></h3>\n`;
  out += code(printed(m), self);
  out += renderMd(docOf(m));
  if (params.length) {
    out += `<div class="table"><table><thead><tr><th>Parameter</th><th>Description</th></tr></thead><tbody>${params
      .map(([p, d]) => `<tr><td><code>${esc(p)}</code></td><td>${(md.parseInline(clean(d), { async: false }) as string)}</td></tr>`)
      .join('')}</tbody></table></div>`;
  }
  if (returns) out += `<p><strong>Returns</strong> ${md.parseInline(clean(returns), { async: false }) as string}</p>`;
  return out;
}

const KIND_LABEL: Record<ApiKind, string> = { class: 'Class', interface: 'Interface', type: 'Type', function: 'Function' };

/** Classes, interfaces, types and functions that mention `name` in their declarations. */
function usedBy(name: string): string[] {
  const re = new RegExp(`\\b${name}\\b`);
  return [...load().items.entries()].filter(([n, { node }]) => n !== name && re.test(printed(node))).map(([n]) => n);
}

export function apiPage(name: string): ApiPage | null {
  const entry = load().items.get(name);
  if (!entry) return null;
  const { item, node } = entry;
  const headings: Heading[] = [];
  const used = new Set<string>();
  let html = `<p class="api-kind">${KIND_LABEL[item.kind]}</p><h1>${esc(name)}</h1>\n`;
  html += renderMd(docOf(node));

  const section = (title: string, members: (ts.ClassElement | ts.TypeElement)[], prefix = '') => {
    if (!members.length) return;
    const id = title.toLowerCase().replace(/\s+/g, '-');
    headings.push({ id, text: title, depth: 2 });
    html += `<h2 id="${id}"><a class="anchor" href="#${id}" aria-hidden="true" tabindex="-1">#</a>${title}</h2>\n`;
    for (const m of members) html += memberSection(m, name, headings, used, prefix);
  };

  if (ts.isClassDeclaration(node)) {
    // Private constructors, `free` and `Symbol.dispose` are wasm-bindgen's memory management, explained once.
    const members = node.members.filter((m) => {
      const n = memberName(m);
      const priv = ts.getCombinedModifierFlags(m as ts.Declaration) & ts.ModifierFlags.Private;
      return !priv && n !== 'free' && n !== '[Symbol.dispose]';
    });
    const isStatic = (m: ts.ClassElement) => !!(ts.getCombinedModifierFlags(m as ts.Declaration) & ts.ModifierFlags.Static);
    const isProp = (m: ts.ClassElement) => ts.isPropertyDeclaration(m) || ts.isGetAccessor(m) || ts.isSetAccessor(m);
    const ctor = members.filter((m) => ts.isConstructorDeclaration(m));
    const statics = members.filter((m) => isStatic(m) && !isProp(m));
    const props = members.filter(isProp);
    const extra = (load().merged.get(name) ?? []).flatMap((i) => [...i.members]);
    const methods = [...members.filter((m) => !isStatic(m) && !isProp(m) && !ts.isConstructorDeclaration(m)), ...extra];
    section('Constructor', ctor);
    section('Static methods', statics, `${name}.`);
    section('Properties', props);
    section('Methods', methods);
    html += `<p class="api-note">Like every object of the API, a <code>${esc(name)}</code> holds WebAssembly memory. Objects returned by the chart are freed with it; <code>free()</code> (or <code>using</code>) releases one earlier.</p>`;
  } else if (ts.isInterfaceDeclaration(node)) {
    const heritage = node.heritageClauses?.map((h) => h.getText()).join(' ');
    if (heritage) html += `<p>${esc(heritage).replace(/(\w+)$/, (n) => (load().items.has(n) ? `<a href="${apiHref(n)}"><code>${n}</code></a>` : n))}</p>`;
    const props = node.members.filter((m) => ts.isPropertySignature(m));
    const methods = node.members.filter((m) => !ts.isPropertySignature(m));
    if (props.length) {
      headings.push({ id: 'properties', text: 'Properties', depth: 2 });
      html += `<h2 id="properties"><a class="anchor" href="#properties" aria-hidden="true" tabindex="-1">#</a>Properties</h2>`;
      html += `<div class="table api-props"><table><thead><tr><th>Name</th><th>Type</th><th>Description</th></tr></thead><tbody>${props
        .map((p) => {
          const ps = p as ts.PropertySignature;
          const id = `prop-${memberName(p)}`;
          const type = ps.type ? code(printed(ps.type), name).replace(/^<div class="code api-sig"><pre><code>|<\/code><\/pre><\/div>$/g, '') : '';
          return `<tr id="${id}"><td><code>${esc(memberName(p))}${ps.questionToken ? '?' : ''}</code></td><td><code class="api-type">${type}</code></td><td>${renderMd(docOf(p))}</td></tr>`;
        })
        .join('')}</tbody></table></div>`;
    }
    section('Methods', methods);
    html += `<h2 id="declaration">Declaration</h2>` + code(printed(node), name);
  } else {
    html += code(printed(node), name);
    const { params, returns } = tagsOf(node);
    if (params.length) {
      html += `<div class="table"><table><thead><tr><th>Parameter</th><th>Description</th></tr></thead><tbody>${params
        .map(([p, d]) => `<tr><td><code>${esc(p)}</code></td><td>${md.parseInline(clean(d), { async: false }) as string}</td></tr>`)
        .join('')}</tbody></table></div>`;
    }
    if (returns) html += `<p><strong>Returns</strong> ${md.parseInline(clean(returns), { async: false }) as string}</p>`;
  }

  const refs = usedBy(name);
  if (refs.length && item.kind !== 'class') {
    headings.push({ id: 'used-by', text: 'Used by', depth: 2 });
    html += `<h2 id="used-by"><a class="anchor" href="#used-by" aria-hidden="true" tabindex="-1">#</a>Used by</h2><p>${refs
      .map((r) => `<a href="${apiHref(r)}"><code>${r}</code></a>`)
      .join(', ')}</p>`;
  }
  return { ...item, html, headings };
}

/** The index page: every item by kind, with its first sentence. */
export function apiIndexHtml(): { html: string; headings: Heading[] } {
  const items = apiItems();
  const groups: [string, ApiItem[]][] = [
    ['Classes', CLASS_ORDER.map((n) => items.find((i) => i.name === n)).filter((i): i is ApiItem => !!i).concat(items.filter((i) => i.kind === 'class' && !CLASS_ORDER.includes(i.name)))],
    ['Functions', items.filter((i) => i.kind === 'function')],
    ['Interfaces', items.filter((i) => i.kind === 'interface').sort((a, b) => a.name.localeCompare(b.name))],
    ['Types', items.filter((i) => i.kind === 'type').sort((a, b) => a.name.localeCompare(b.name))],
  ];
  const headings: Heading[] = [];
  let html = `<h1>JavaScript API reference</h1>
<p>Every class, function and type of <code>@ferrochart/core</code>, generated from the TypeScript declarations shipped in the package. For how to use them, see the guide chapters; for loading the module and general conventions, see <a href="/docs/js-api">JavaScript API overview</a>.</p>`;
  for (const [title, list] of groups) {
    if (!list.length) continue;
    const id = title.toLowerCase();
    headings.push({ id, text: title, depth: 2 });
    html += `<h2 id="${id}"><a class="anchor" href="#${id}" aria-hidden="true" tabindex="-1">#</a>${title}</h2><div class="table api-index"><table><tbody>${list
      .map((i) => `<tr><td><a href="${apiHref(i.name)}"><code>${i.name}</code></a></td><td>${md.parseInline(i.summary, { async: false }) as string}</td></tr>`)
      .join('')}</tbody></table></div>`;
  }
  return { html, headings };
}
