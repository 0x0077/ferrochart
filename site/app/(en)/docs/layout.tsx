import type { ReactNode } from 'react';
import { DocsLayout } from '@/components/pages/DocsPages';

export default function Layout({ children }: { children: ReactNode }) {
  return <DocsLayout locale="en">{children}</DocsLayout>;
}
