import { DocsIndex, docsIndexMeta } from '@/components/pages/DocsPages';

export const metadata = docsIndexMeta('en');

export default function Page() {
  return <DocsIndex locale="en" />;
}
