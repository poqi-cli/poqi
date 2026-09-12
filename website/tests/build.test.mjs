import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const output = new URL('../dist/', import.meta.url);
const origin = 'https://poqi-cli.github.io';
const base = '/poqi/';
const home = `${origin}${base}`;
const html = await readFile(new URL('index.html', output), 'utf8');
const guide = await readFile(new URL('getting-started/index.html', output), 'utf8');
const errorHtml = await readFile(new URL('404.html', output), 'utf8');

async function assertLocalUrlExists(value) {
  const url = new URL(value.replaceAll('&amp;', '&'), home);
  if (url.origin !== origin) return;
  assert.ok(url.pathname.startsWith(base), `URL escapes project base: ${value}`);
  const relative = decodeURIComponent(url.pathname.slice(base.length));
  const file = relative.endsWith('/') || !relative ? `${relative}index.html` : relative;
  assert.ok((await stat(new URL(file, output))).isFile(), `Missing built asset: ${value}`);
  if (url.hash && file.endsWith('.html')) {
    const target = await readFile(new URL(file, output), 'utf8');
    assert.ok(target.includes(`id="${decodeURIComponent(url.hash.slice(1))}"`), `Missing linked section: ${value}`);
  }
}

test('all emitted local links, image candidates and scripts resolve under the Pages base', async () => {
 for (const document of [html, guide, errorHtml]) {
  for (const match of document.matchAll(/\b(?:href|src)="([^"#][^"]*)"/g)) {
    if (!/^(?:data:|mailto:|tel:)/.test(match[1])) await assertLocalUrlExists(match[1]);
  }
  for (const match of document.matchAll(/\bsrcset="([^"]+)"/g)) {
    for (const candidate of match[1].split(',')) await assertLocalUrlExists(candidate.trim().split(/\s/)[0]);
  }
  for (const match of document.matchAll(/href="#([^"]+)"/g)) assert.ok(document.includes(`id="${match[1]}"`), `Missing anchor ${match[1]}`);
 }
});

test('indexable HTML contains canonical, social metadata and real product content without JavaScript', async () => {
  assert.match(html, /<html[^>]+lang="en"/);
  assert.match(html, /<title>[^<]*PostgreSQL[^<]*<\/title>/);
  assert.equal((html.match(/<h1(?:\s|>)/g) || []).length, 1);
  assert.match(html, /<link[^>]+rel="canonical"[^>]+href="https:\/\/poqi-cli\.github\.io\/poqi\/"/);
  assert.doesNotMatch(html, /noindex|localhost|127\.0\.0\.1|C:\\repositories/);
  for (const tag of ['description','og:title','og:description','og:image','og:url','twitter:card']) assert.ok(html.includes(`"${tag}"`), tag);
  for (const text of ['PostgreSQL', 'Windows', 'macOS', 'Linux', 'No-Sale Source License']) assert.ok(html.includes(text), text);
  const json = [...html.matchAll(/<script[^>]*type="application\/ld\+json"[^>]*>([\s\S]*?)<\/script>/g)].map(m => JSON.parse(m[1]));
  assert.ok(json.length, 'Missing structured data');
  assert.match(JSON.stringify(json), /SoftwareApplication/);
  assert.doesNotMatch(JSON.stringify(json), /aggregateRating|reviewCount/);
  const app = json.find(item => item['@type'] === 'SoftwareApplication');
  assert.equal(app.offers.price, 0);
  assert.match(html, /Free download/);
  assert.equal(app.isAccessibleForFree, true);
  assert.ok(!('codeRepository' in app) && !('programmingLanguage' in app), 'Source-code properties do not belong to SoftwareApplication');
  assert.ok(app.downloadUrl.length === 4 && app.downloadUrl.every(url => /releases\/download\/.+\.(zip|gz)$/.test(url)));
  await assertLocalUrlExists(app.softwareHelp.url);
  await assertLocalUrlExists(app.image);
});

test('the installation guide has distinct metadata, real instructions and matching breadcrumbs', async () => {
  assert.match(guide, /<title>Install poqi and connect to PostgreSQL/);
  assert.match(guide, /rel="canonical" href="https:\/\/poqi-cli\.github\.io\/poqi\/getting-started\/"/);
  assert.doesNotMatch(guide, /noindex/);
  for (const text of ['Install poqi','Connect to PostgreSQL','Run SQL','SELECT current_database()', '--check-connection', 'Secret Service']) assert.ok(guide.includes(text), text);
  const data = JSON.parse(guide.match(/<script[^>]*type="application\/ld\+json"[^>]*>([\s\S]*?)<\/script>/)[1]);
  assert.equal(data.breadcrumb['@type'], 'BreadcrumbList');
  assert.equal(data.breadcrumb.itemListElement[1].item, `${home}getting-started/`);
  for (const item of data.breadcrumb.itemListElement) await assertLocalUrlExists(item.item);
});

test('sitemap exposes the canonical homepage and excludes error pages', async () => {
  const index = await readFile(new URL('sitemap-index.xml', output), 'utf8');
  const maps = [...index.matchAll(/<loc>([^<]+)<\/loc>/g)].map(m => m[1]);
  assert.ok(maps.length);
  let urls = [];
  for (const map of maps) {
    await assertLocalUrlExists(map);
    const xml = await readFile(fileURLToPath(new URL(new URL(map).pathname.slice(base.length), output)), 'utf8');
    urls.push(...[...xml.matchAll(/<loc>([^<]+)<\/loc>/g)].map(m => m[1]));
  }
  assert.ok(urls.includes(home));
  assert.ok(urls.includes(`${home}getting-started/`));
  assert.deepEqual([...urls].sort(), [home, `${home}getting-started/`].sort(), 'Only canonical HTML pages belong in the sitemap');
  for (const url of urls) await assertLocalUrlExists(url);
  const error = await readFile(new URL('404.html', output), 'utf8');
  assert.match(error, /noindex/);
  assert.doesNotMatch(error, /rel="canonical"|property="og:url"/);
  const robots = await readFile(new URL('robots.txt', output), 'utf8');
  assert.ok(robots.includes(`${home}sitemap-index.xml`));
  assert.doesNotMatch(robots, /^Disallow:\s*\/$/m);
});

test('AI index is discoverable and release documentation matches the actual downloads', async () => {
  const { version, platforms, downloadUrl } = await import('../src/data/release.js');
  const index = await readFile(new URL('llms.txt', output), 'utf8');
  assert.match(index, /^# poqi\n\n> /);
  assert.ok(index.includes(`Current website download version: ${version}`));
  assert.doesNotMatch(index, /localhost|127\.0\.0\.1|C:\\repositories|<html/i);
  for (const doc of [html, guide]) assert.match(doc, /rel="describedby" type="text\/plain" href="\/poqi\/llms\.txt"/);
  assert.doesNotMatch(errorHtml, /rel="describedby"/);
  for (const platform of Object.values(platforms)) assert.ok(index.includes(downloadUrl(platform)));
  for (const name of ['README.md', 'docs/connections.md', 'docs/current-status.md']) {
    assert.ok(index.includes(`raw.githubusercontent.com/poqi-cli/poqi/${version}/${name}`));
  }
  const links = [...index.matchAll(/\]\((https:\/\/[^)]+)\)/g)].map(match => match[1]);
  assert.ok(links.includes(home) && links.includes(`${home}getting-started/`));
  for (const url of links) await assertLocalUrlExists(url);
});
