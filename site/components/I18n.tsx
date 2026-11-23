'use client';
// The language of the page for client components: its interface text and,
// where examples are shown, the catalog in that language (one copy, so the
// demo objects are the same everywhere on the page).
import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { categories as english, type Category, type Demo } from '@/catalog';
import { localize, type CatalogText } from '@/catalog/localize';
import { localePath, type Locale } from '@/lib/i18n';
import type { Messages } from '@/lib/messages';

type I18n = { locale: Locale; t: Messages; path: (p: string) => string };
const I18nContext = createContext<I18n | null>(null);

export function I18nProvider({ locale, t, children }: { locale: Locale; t: Messages; children: ReactNode }) {
  const value = useMemo(() => ({ locale, t, path: (p: string) => localePath(locale, p) }), [locale, t]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  const v = useContext(I18nContext);
  if (!v) throw new Error('useI18n outside I18nProvider');
  return v;
}

type Catalog = { categories: Category[]; allDemos: Demo[]; findDemo: (id: string) => Demo | undefined };
const CatalogContext = createContext<Catalog | null>(null);

/**
 * The catalog of the page's language. A translated page carries the titles;
 * the descriptions load from /i18n/<locale>.json after the page (until then
 * they show in English). They are filled into the same demo objects, which
 * the viewer compares by identity.
 */
export function CatalogProvider({ text, children }: { text: CatalogText | null; children: ReactNode }) {
  const { locale } = useI18n();
  const base = useMemo(() => {
    const categories = localize(english, text);
    const allDemos = categories.flatMap((c) => c.demos);
    return { categories, allDemos, findDemo: (id: string) => allDemos.find((d) => d.id === id) };
  }, [text]);
  const [version, setVersion] = useState(0);
  useEffect(() => {
    if (locale === 'en') return;
    let gone = false;
    fetch(`/i18n/${locale}.json`)
      .then((r) => (r.ok ? (r.json() as Promise<Record<string, string>>) : null))
      .then((abouts) => {
        if (gone || !abouts) return;
        for (const d of base.allDemos) if (abouts[d.id]) d.about = abouts[d.id];
        setVersion((v) => v + 1);
      })
      .catch(() => {});
    return () => {
      gone = true;
    };
  }, [locale, base]);
  const value = useMemo(() => ({ ...base, version }), [base, version]);
  return <CatalogContext.Provider value={value}>{children}</CatalogContext.Provider>;
}

export function useCatalog(): Catalog {
  const v = useContext(CatalogContext);
  if (!v) throw new Error('useCatalog outside CatalogProvider');
  return v;
}
