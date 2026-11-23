import { Home } from '@/components/Home';
import { localePath, type Locale } from '@/lib/i18n';
import { messages } from '@/lib/messages';
import { REPO_URL, SITE_NAME, SITE_URL, SITE_VERSION } from '@/lib/seo';

/** The homepage, with structured data for search engines: the project and the site. */
export function HomePage({ locale }: { locale: Locale }) {
  const url = SITE_URL + (locale === 'en' ? '' : localePath(locale, '/'));
  const jsonLd = [
    {
      '@context': 'https://schema.org',
      '@type': 'SoftwareSourceCode',
      name: SITE_NAME,
      description: messages(locale).home.jsonLd,
      url,
      codeRepository: REPO_URL,
      programmingLanguage: ['Rust', 'TypeScript', 'Python'],
      runtimePlatform: ['Web browser (WebGPU, WebGL2)', 'Windows', 'macOS', 'Linux'],
      license: 'https://www.apache.org/licenses/LICENSE-2.0',
      version: SITE_VERSION,
    },
    { '@context': 'https://schema.org', '@type': 'WebSite', name: SITE_NAME, url },
  ];
  return (
    <>
      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd).replace(/</g, '\\u003c') }} />
      <Home locale={locale} />
    </>
  );
}
