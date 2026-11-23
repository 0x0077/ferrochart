/**
 * Site-wide settings.
 *
 * `githubRepo`: the repository the header's star button links to and counts.
 * The site is a static export: the HTML has the count at build time, and the
 * page updates it from the data API (`/api/github/stars`).
 */
export const site = {
  githubRepo: '0x0077/ferrochart',
};
