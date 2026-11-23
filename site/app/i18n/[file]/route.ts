// The example descriptions of each language (/i18n/zh.json): pages carry
// only the titles and fetch the descriptions when they need them, so 1,200
// translated pages do not each repeat all the descriptions.
import { catalogAbouts } from '@/lib/catalog-text';
import { PREFIXED, type Locale } from '@/lib/i18n';

export const dynamic = 'force-static';
export const dynamicParams = false;

export function generateStaticParams() {
  return PREFIXED.map((l) => ({ file: `${l}.json` }));
}

export async function GET(_: Request, { params }: { params: Promise<{ file: string }> }) {
  const locale = (await params).file.replace(/\.json$/, '') as Locale;
  return Response.json(catalogAbouts(locale));
}
