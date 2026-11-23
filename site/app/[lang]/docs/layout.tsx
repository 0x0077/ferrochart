import type { ReactNode } from 'react';
import { DocsLayout } from '@/components/pages/DocsPages';
import type { Locale } from '@/lib/i18n';

export default async function Layout({ children, params }: { children: ReactNode; params: Promise<{ lang: string }> }) {
  return <DocsLayout locale={(await params).lang as Locale}>{children}</DocsLayout>;
}
