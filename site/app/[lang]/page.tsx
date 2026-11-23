import { HomePage } from '@/components/pages/HomePage';
import type { Locale } from '@/lib/i18n';

export default async function Page({ params }: { params: Promise<{ lang: string }> }) {
  return <HomePage locale={(await params).lang as Locale} />;
}
