import lighthouse from 'lighthouse';
import desktop from 'lighthouse/core/config/desktop-config.js';
import { launch } from 'chrome-launcher';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

// Start `npm run preview` first, or pass the published URL.
const url = process.argv[2] || 'http://127.0.0.1:4321/poqi/';
if (!['http:', 'https:'].includes(new URL(url).protocol)) throw new Error('Expected an HTTP(S) URL');
const pageKey = new URL(url).pathname.replace(/^\/|\/$/g, '').replace(/[^a-zA-Z0-9_-]/g, '-') || 'root';
const output = new URL(`../../artifacts/website-lighthouse/${pageKey}/`, import.meta.url);
await mkdir(output, {recursive:true});
const summary = [];
for (const mode of ['mobile', 'desktop']) {
  for (let run = 1; run <= 3; run++) {
    // Own only this newly created profile. Retried cleanup handles Windows file locks
    // while Chrome exits; no browser belonging to the user is reused or stopped.
    const profile = await mkdtemp(fileURLToPath(new URL('chrome-', output)));
    let chrome;
    try {
      chrome = await launch({userDataDir:profile,
        chromeFlags:['--headless=new', '--no-first-run', '--disable-extensions']});
      const result = await lighthouse(url, {port:chrome.port, output:['html','json'], logLevel:'error',
        onlyCategories:['performance','accessibility','best-practices','seo']}, mode === 'desktop' ? desktop : undefined);
      if (!result || result.lhr.runtimeError) throw new Error(JSON.stringify(result?.lhr.runtimeError || 'No report'));
      const report = result.lhr;
      await writeFile(new URL(`${mode}-${run}.report.html`, output), result.report[0]);
      await writeFile(new URL(`${mode}-${run}.report.json`, output), result.report[1]);
      const entry = {mode, run, url:report.finalDisplayedUrl, lighthouse:report.lighthouseVersion,
        scores:Object.fromEntries(Object.entries(report.categories).map(([key, value]) => [key, Math.round(value.score * 100)])),
        lcpMs:report.audits['largest-contentful-paint'].numericValue,
        cls:report.audits['cumulative-layout-shift'].numericValue,
        tbtMs:report.audits['total-blocking-time'].numericValue};
      summary.push(entry);
      console.log(JSON.stringify(entry));
    } finally {
      await chrome?.kill();
      await rm(profile, {recursive:true, force:true, maxRetries:30, retryDelay:100});
    }
  }
}
await writeFile(new URL('summary.json', output), JSON.stringify(summary, null, 2) + '\n');
console.log(`Reports: ${fileURLToPath(output)}`);
