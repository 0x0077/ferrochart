import { site } from '@/lib/site';
import { GitHubStarLink } from './GitHubStarLink';

/**
 * Star count of `repo` at build time; `null` when GitHub cannot be reached.
 * The site is a static export, so the page refreshes it after loading
 * (`GitHubStarLink`).
 */
async function stars(repo: string): Promise<number | null> {
  try {
    const r = await fetch(`https://api.github.com/repos/${repo}`, {
      headers: { Accept: 'application/vnd.github+json', 'User-Agent': 'ferrochart-site' },
      next: { revalidate: 60 },
    });
    if (!r.ok) return null;
    const n = (await r.json()).stargazers_count;
    return typeof n === 'number' ? n : null;
  } catch {
    return null;
  }
}

/**
 * The header's "Star on GitHub" button: the GitHub mark, "Star" and the
 * count. The HTML carries the count at build time; the page then asks the
 * data API (`/api/github/stars`, cached there) for the current one, so the
 * visitor's browser never contacts GitHub.
 */
export async function GitHubStar({ label, labelCount }: { label: string; labelCount: string }) {
  const repo = site.githubRepo;
  return <GitHubStarLink repo={repo} initial={await stars(repo)} label={label} labelCount={labelCount} />;
}
