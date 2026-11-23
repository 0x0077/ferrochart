// The languages of the site. English has no path prefix (/docs); every other
// language lives under its own (/zh/docs). See i18n/TRANSLATING.md.

export const LOCALES = ['en', 'zh', 'zh-tw', 'ja', 'ko'] as const;
export type Locale = (typeof LOCALES)[number];
/** The languages with a path prefix. */
export const PREFIXED = LOCALES.filter((l) => l !== 'en') as Exclude<Locale, 'en'>[];

/** `<html lang>` and hreflang of each locale. */
export const HTML_LANG: Record<Locale, string> = { en: 'en', zh: 'zh-CN', 'zh-tw': 'zh-TW', ja: 'ja', ko: 'ko' };
/** Open Graph locale. */
export const OG_LOCALE: Record<Locale, string> = { en: 'en_US', zh: 'zh_CN', 'zh-tw': 'zh_TW', ja: 'ja_JP', ko: 'ko_KR' };
/** Each language's name in itself, for the language menu. */
export const LANGUAGE_NAME: Record<Locale, string> = { en: 'English', zh: '简体中文', 'zh-tw': '繁體中文', ja: '日本語', ko: '한국어' };

export const isLocale = (s: string): s is Locale => (LOCALES as readonly string[]).includes(s);

/** Paths that exist in English only (the API reference, generated from the source). */
export const englishOnly = (path: string) => /^\/docs\/api(\/|$)/.test(path) || path.startsWith('/thumb');

/**
 * The address of `path` (an English path: "/", "/docs/web", "/#library") in
 * `locale`. English-only pages keep their English address.
 */
export function localePath(locale: Locale, path: string): string {
  if (locale === 'en' || englishOnly(path) || /^[a-z]+:/.test(path)) return path;
  if (path === '/') return `/${locale}`;
  if (path.startsWith('/#')) return `/${locale}${path.slice(1)}`;
  return `/${locale}${path}`;
}

/** Splits a site path into its locale and the English path: "/zh/docs" → ["zh", "/docs"]. */
export function splitPath(path: string): [Locale, string] {
  const m = path.match(/^\/([a-z]{2}(?:-[a-z]{2})?)(?=\/|$)(.*)$/);
  if (m && isLocale(m[1]) && m[1] !== 'en') return [m[1], m[2] || '/'];
  return ['en', path || '/'];
}

/** Fills `{name}` placeholders. */
export function fmt(s: string, vars: Record<string, string | number>): string {
  return s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
}
