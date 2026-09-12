import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';

const site = 'https://poqi-cli.github.io';
const base = '/poqi';

export default defineConfig({
  site,
  base,
  output: 'static',
  trailingSlash: 'always',
  integrations: [
    sitemap({
      filter: (page) => page !== `${site}${base}/404/` && page !== `${site}${base}/404.html`,
    }),
  ],
});
