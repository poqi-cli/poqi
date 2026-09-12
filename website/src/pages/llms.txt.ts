import { repo, version, platforms, downloadUrl, licenseUrl } from '../data/release.js';
import type { APIRoute } from 'astro';

export const GET: APIRoute = ({ site }) => {
  const home = new URL(import.meta.env.BASE_URL, site).href;
  const raw = `${repo.replace('github.com', 'raw.githubusercontent.com')}/${version}`;
  const text = `# poqi

> poqi (PostgreSQL Query Interface) is a keyboard-first PostgreSQL terminal client and SQL editor for Windows, macOS and Linux.

This index covers ${home}. Current website download version: ${version}.
Downloads are free under the poqi No-Sale Source License 1.0; the source is available, but the license is not an OSI open-source license.
poqi browses schemas, runs SQL and supports eligible single-table row updates and deletes. Optional local semantic search ranks fetched rows; it is disabled by default and requires model and runtime downloads when enabled.

## Website

- [Product overview](${home}): Features, real application screenshots, six themes and release downloads.
- [Getting started](${home}getting-started/): Installation, PostgreSQL connections, SQL, keyboard controls and troubleshooting.
- [XML sitemap](${home}sitemap-index.xml): Canonical indexable website pages.

## Release documentation (Markdown)

- [README](${raw}/README.md): Product overview and installation instructions for ${version}.
- [Connections](${raw}/docs/connections.md): Profiles, connection URLs, TLS and credential storage.
- [Implemented features and limitations](${raw}/docs/current-status.md): Verified application scope for this release.

## Downloads

${Object.values(platforms).map(platform => `- [${platform.title}](${downloadUrl(platform)}): ${platform.meta}. ${platform.note}`).join('\n')}

## Optional

- [Source repository](${repo}): Current development source and issue tracker.
- [License](${licenseUrl}): License terms for the reviewed release.
`;
  return new Response(text, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
};
