# poqi website

Astro source for the approved white/cobalt product website. It builds static files for `https://poqi-cli.github.io/poqi/`.

This is the maintained website implementation. See [DESIGN.md](DESIGN.md) for the selected visual direction and screenshot provenance.

## Development and validation

Use Node.js 22.19 or newer. Node.js 24 is recommended and used by CI.

```sh
npm ci
npm run dev
```

Open `http://localhost:4321/poqi/`. For production checks:

```sh
npm run build
npm test
npx playwright install chromium
npm run test:browser
```

The build writes deployable files to `dist/`. The browser suite starts its own preview server. Set `PLAYWRIGHT_CHANNEL=chrome` to use an installed Chrome instead of downloading Chromium.

For visual review and Lighthouse, start `npm run preview` and open `http://127.0.0.1:4321/poqi/`. In another terminal, run `npm run audit`. This requires Chrome (set `CHROME_PATH` if it is not discovered automatically), performs three runs for each default mobile and desktop Lighthouse preset, and writes reports to the repository's ignored `artifacts/website-lighthouse/` directory. To measure the published site, use `npm run audit -- https://poqi-cli.github.io/poqi/`. On Windows, use `npm.cmd` if PowerShell's npm wrapper consumes arguments after `--`.

## Website analytics

The published site uses free [Cloudflare Web Analytics](https://developers.cloudflare.com/web-analytics/about/). In the Cloudflare dashboard, open **Analytics → Web analytics → poqi-cli.github.io** to view visits, page views, referrers, countries, devices and page performance. Filter by `/poqi/` paths when needed. Collection starts after installation; these metrics do not identify individual visitors or prove completed downloads or installations. GitHub release asset download counts remain separate.

The shared layout loads Cloudflare's module beacon only on `https://poqi-cli.github.io/poqi/` and its subpages. Local previews and copies hosted elsewhere do not send analytics. The beacon's site token is a public identifier intended for page source, not an account API credential. To change the destination site, copy its token from Cloudflare's **Manage site** snippet and update the layout's origin/path guard together. Removing the beacon block disables collection. No DNS or hosting migration is needed.

## Hosting and search indexing

The Astro `site`, `base`, and trailing-slash settings target the GitHub Pages project site. In repository Settings → Pages, set Source to **GitHub Actions**. Follow the repository's feature-branch → `dev` → `main` contribution flow. The `Website` workflow validates pull requests and publishes only `website/dist/` from `main` in `poqi-cli/poqi`. It can also be dispatched manually on `main`. Its Node build and deployment are separate from the Rust release workflow. The website package itself does not publish anything.

After publishing, verify the HTTPS homepage, `/poqi/getting-started/`, and a nonexistent path (HTTP 404), then add `https://poqi-cli.github.io/poqi/` as a URL-prefix property in Google Search Console. Choose HTML-tag verification and set the actual tag's `content` token as the GitHub repository Actions variable `GOOGLE_SITE_VERIFICATION`. Dispatch the Website workflow on `main`, then complete verification in Search Console. The build emits the verification meta tag only when the variable is present. Submit `https://poqi-cli.github.io/poqi/sitemap-index.xml` and use URL Inspection on both pages to check indexing eligibility. No Google account or token is bundled here. Also set the repository About website field to the published URL after the deployment succeeds.

`public/robots.txt` is emitted at `/poqi/robots.txt`. Crawlers only treat `/robots.txt` at the domain root as authoritative: this project-path file advertises the sitemap but cannot set domain-wide crawl policy. Submit the sitemap directly in Search Console. A missing root robots file does not itself block indexing. If an organization root site becomes available, its root robots file can advertise this sitemap. The 404 page has `noindex` metadata and is excluded from the sitemap.

For a custom domain, update `site`, `base`, and the sitemap URL in `public/robots.txt`, configure Pages DNS/HTTPS, and adjust the URL contract in the tests together.

Lighthouse's SEO score checks technical basics; it is not a Google ranking score. Useful product content, accurate documentation, discovery links and real-user performance still matter. Remeasure the hosted site after deployment because local results do not establish CDN, network or field performance. See [Google's SEO guide](https://developers.google.com/search/docs/fundamentals/seo-starter-guide) and [Astro's Pages guide](https://docs.astro.build/en/guides/deploy/github/).

The application JSON-LD describes the free downloads, current release, screenshots, license and help page using schema.org properties. It does **not** establish Google's software-app rich-result eligibility: that feature additionally requires a real visible review or rating, which this site does not have. Never invent one to clear a validator. The guide has a matching visible breadcrumb and `BreadcrumbList`. See [Google's software-app requirements](https://developers.google.com/search/docs/appearance/structured-data/software-app).

Website/documentation-only changes, including the website/release trigger configuration and its scope regression test, skip binary publication. Mixing an application or packaging change into the same `dev` → `main` PR still invokes the existing version/tag guard; update the application version before that release. Pages builds and publishes from `main` independently.

## AI discovery

`/poqi/llms.txt` is a concise, build-generated index of the website, release-matched Markdown documentation and downloads. Indexable pages advertise it with `rel="describedby"`. Its version and archive links use the same release data as the UI. It follows the [llms.txt proposal](https://llmstxt.org/), which permits a project subpath; it is not a crawler permission file or a promise of inclusion in AI answers.

For ChatGPT search, allow `OAI-SearchBot` at the domain root and through any hosting/firewall rules. OpenAI documents it separately from the training crawler `GPTBot` and user-triggered `ChatGPT-User`; search visibility does not require enabling training. This project does not add restrictions, but `/poqi/robots.txt` cannot override a host's `/robots.txt`. Verify the root policy and live page accessibility after publishing. See [OpenAI crawler documentation](https://developers.openai.com/api/docs/bots).

Google does not use llms.txt for Search or its generative AI features. Keep the ordinary XML sitemap, accessible rendered HTML, descriptive internal links and accurate visible content as the primary discovery path. After deployment, verify Search Console indexing and the current generative-AI inclusion setting if available for the property. See [Google's current AI optimization guidance](https://developers.google.com/search/docs/fundamentals/ai-optimization-guide). No artificial keyword pages, ratings, hidden instructions or unverified ranking claims are included.

## Release updates and assets

The download UI describes v1.0.2. Verify published asset names and sizes before updating `src/data/release.js`; the homepage, guide, client tabs and application metadata share it. Keep the guide's behavior descriptions aligned with `docs/connections.md` and `docs/current-status.md`. Archive links stay pinned to the reviewed release; optional terminal installer commands use the project's official `latest` assets.

The primary Windows download is `poqi-v1.0.2-windows-x86_64-setup.exe`; the ZIP remains the portable alternative. The Windows instructions cover opening setup and launching poqi from Start, including the unsigned-publisher notice. For each update, verify the published asset checksum and size before changing the shared release data.

Keep platform summaries short and put compatibility and credential-store details in the guide's system requirements. The verified v1.0.2 Linux archive's ELF version requirements include `GLIBC_2.39`; this is a binary requirement, not merely the build runner's version. Verify it again when updating releases. The main download button has a fixed size across platform choices; its accessible name preserves the selected Mac processor.

The distribution is source available under the poqi No-Sale Source License 1.0. Keep that label and link accurate; do not describe the project as OSI open source.

Original PNG screenshots stay in `src/assets/` for full-size viewing. Astro generates responsive WebP variants for the page, including all six themes. Screenshots show a real terminal runtime with synthetic demonstration data; they are not performance benchmarks.

The short videos in `public/demos/` record the published v1.0.1 Linux application using synthetic PostgreSQL product data. Terminal output was captured and rendered at 1044 × 608, 12 fps, without audio. The semantic demo used the supported `ORT_DYLIB_PATH` override to the checksum-verified versioned ONNX library because v1.0.1 selected an archive symlink during automatic Linux setup. The recordings demonstrate application behavior, not setup or benchmark timing. Keep the WebM, MP4 and poster variants together. Native controls and `preload="none"` defer video bytes until playback; supporting browsers also defer posters through `loading="lazy"`.
