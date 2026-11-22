import { Home } from '@/components/Home';
import { REPO_URL, SITE_NAME, SITE_URL } from '@/lib/seo';

/** Structured data for search engines: the project and the site. */
const jsonLd = [
  {
    '@context': 'https://schema.org',
    '@type': 'SoftwareSourceCode',
    name: SITE_NAME,
    description: 'Open-source, GPU-rendered financial charting engine written in Rust, for the web, desktop, Python and C.',
    url: SITE_URL,
    codeRepository: REPO_URL,
    programmingLanguage: ['Rust', 'TypeScript', 'Python'],
    runtimePlatform: ['Web browser (WebGPU, WebGL2)', 'Windows', 'macOS', 'Linux'],
    license: 'https://www.apache.org/licenses/LICENSE-2.0',
    version: '0.3.0',
  },
  {
    '@context': 'https://schema.org',
    '@type': 'WebSite',
    name: SITE_NAME,
    url: SITE_URL,
  },
];

export default function HomePage() {
  return (
    <>
      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd).replace(/</g, '\\u003c') }} />
      <Home />
    </>
  );
}
