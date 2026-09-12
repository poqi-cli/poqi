import { test, expect } from '@playwright/test';

test('silent demos wait for playback and work without JavaScript', async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  const requested = [];
  page.on('request', request => {
    if (/\.(webm|mp4)(?:\?|$)/.test(request.url())) requested.push(request.url());
  });
  await page.goto('http://127.0.0.1:4321/poqi/');
  const videos = page.locator('.demo-video video');
  await expect(videos).toHaveCount(3);
  for (const video of await videos.all()) await video.scrollIntoViewIfNeeded();
  expect(requested).toEqual([]);
  for (const video of await videos.all()) {
    await expect(video).toHaveAttribute('preload', 'none');
    expect(await video.evaluate(v => v.autoplay)).toBe(false);
    const before = await video.boundingBox();
    // Click Chromium's native play control with page scripts disabled.
    await video.click({ position: { x: 29, y: before.height - 53 } });
    await expect.poll(() => video.evaluate(v => v.currentTime)).toBeGreaterThan(0.2);
    expect(await video.evaluate(v => v.getVideoPlaybackQuality().totalVideoFrames)).toBeGreaterThan(0);
    expect((await video.boundingBox()).height).toBe(before.height);
    await video.evaluate(v => v.pause());
  }
  expect(requested.some(url => url.includes('semantic-search.webm'))).toBe(true);
  await context.close();
});

for (const [url, enabled] of [
  ['https://poqi-cli.github.io/poqi/', true],
  ['https://poqi-cli.github.io/poqi/getting-started/', true],
  ['http://127.0.0.1:4321/poqi/', false],
  ['https://example.invalid/poqi/', false],
  ['https://poqi-cli.github.io/other-project/', false],
]) {
  test(`analytics destination is restricted at ${url}`, async ({ page, request }) => {
    const beacons = [];
    // Exercise the built pages under real-looking origins without sending telemetry.
    await page.route('**/*', async route => {
      const target = new URL(route.request().url());
      if (target.href === 'https://static.cloudflareinsights.com/beacon.min.js') {
        beacons.push(target.href);
        await route.fulfill({contentType:'text/javascript', body:'window.testBeaconLoaded = true;'});
      } else if (target.origin === new URL(url).origin) {
        const path = target.pathname.startsWith('/poqi/') ? target.pathname : '/poqi/';
        await route.fulfill({response:await request.get(`http://127.0.0.1:4321${path}${target.search}`)});
      } else {
        await route.abort();
      }
    });
    await page.goto(url);
    const beacon = page.locator('script[data-cf-beacon]');
    await expect(beacon).toHaveCount(enabled ? 1 : 0);
    if (enabled) {
      await expect.poll(() => page.evaluate(() => window.testBeaconLoaded)).toBe(true);
      await expect(beacon).toHaveAttribute('type', 'module');
      expect(JSON.parse(await beacon.getAttribute('data-cf-beacon'))).toEqual({token:'bc5555ffaf89411e90c97af199873e80'});
    }
    expect(beacons).toHaveLength(enabled ? 1 : 0);
    // Drain asset handlers before Playwright disposes the request fixture.
    await page.unrouteAll({behavior:'wait'});
  });
}

for (const width of [360, 390, 768, 1280, 1440]) {
  test(`responsive assets and anchors at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto('/poqi/');
    await page.evaluate(async () => {
      await Promise.all([...document.images].filter(i => i.getAttribute('src')).map(i => { i.loading = 'eager'; return i.decode(); }));
    });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    expect(errors).toEqual([]);
    await expect(page.locator('h1')).toContainText('Your database.');
  });
}

test('theme previews, modal focus, platform keyboard navigation and clipboard', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/poqi/');
  const initial = await page.locator('#theme-image').getAttribute('src');
  for (const theme of ['light', 'cyberpunk', 'midnight', 'nord', 'synthwave', 'dark']) {
    const button = page.locator(`button[data-theme="${theme}"]`);
    await button.click();
    await expect(button).toHaveAttribute('aria-pressed', 'true');
    await expect(page.locator('#theme-image')).toHaveJSProperty('complete', true);
    expect(await page.locator('#theme-image').evaluate(i => i.naturalWidth)).toBeGreaterThan(0);
    if (theme !== 'dark') expect(await page.locator('#theme-image').getAttribute('src')).not.toBe(initial);
  }
  await page.locator('#main-full').click();
  await expect(page.locator('#image-viewer')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('#image-viewer')).not.toBeVisible();
  await expect(page.locator('#main-full')).toBeFocused();
  for (const [platform, file, label] of [['windows','windows-x86_64-setup.exe','Windows'],['macos-arm','macos-arm64.tar.gz','macOS Apple Silicon'],['macos-intel','macos-x86_64.tar.gz','macOS Intel'],['linux','linux-x86_64.tar.gz','Linux x64']]) {
    await page.locator(`[data-platform="${platform}"]`).click();
    await expect(page.locator('#download-link')).toHaveAttribute('href', new RegExp(`${file.replaceAll('.', '\\.')}$`));
    await expect(page.locator('#download-link')).toHaveAttribute('aria-label', `Download for ${label}`);
  }
  await expect(page.getByRole('link', {name:'Portable Windows ZIP'})).toHaveAttribute('href', /windows-x86_64\.zip$/);
  await page.locator('[data-platform="linux"]').focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.locator('[data-platform="windows"]')).toHaveAttribute('aria-selected', 'true');
  await page.locator('.installer-details summary').click();
  await page.locator('#copy-command').click();
  await expect(page.locator('#copy-status')).toContainText('copied');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(await page.locator('#install-command').textContent());
});

test('content and all release platforms remain available without JavaScript', async ({ browser }) => {
  const context = await browser.newContext({javaScriptEnabled:false});
  const page = await context.newPage();
  await page.goto('http://127.0.0.1:4321/poqi/');
  await expect(page.locator('h1')).toContainText('Your database.');
  for (const file of ['windows-x86_64-setup.exe','macos-arm64.tar.gz','macos-x86_64.tar.gz','linux-x86_64.tar.gz']) {
    await expect(page.locator(`a[href$="${file}"]`).first()).toBeVisible();
  }
  await expect(page.getByRole('link', {name:'Portable Windows ZIP'})).toHaveAttribute('href', /windows-x86_64\.zip$/);
  await context.close();
});

test('unknown paths return a noindex 404 with working homepage navigation', async ({ page }) => {
  const response = await page.goto('/poqi/this-page-does-not-exist/');
  expect(response.status()).toBe(404);
  await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', /noindex/);
  await page.getByRole('link', {name:/Return to poqi/}).click();
  await expect(page).toHaveURL(/\/poqi\/$/);
  await expect(page.locator('h1')).toContainText('Your database.');
});

test('guide navigation and installer details work on a narrow screen without JavaScript', async ({ browser }) => {
  const context = await browser.newContext({javaScriptEnabled:false, reducedMotion:'reduce', viewport:{width:360,height:800}});
  const page = await context.newPage();
  await page.goto('http://127.0.0.1:4321/poqi/');
  await page.getByRole('link', {name:'Installation & connection guide →'}).click();
  await expect(page).toHaveURL(/\/poqi\/getting-started\/$/);
  await expect(page.getByRole('heading', {level:1})).toContainText('Get started');
  await expect(page.getByRole('link', {name:'Download for Windows'})).toHaveAttribute('href', /windows-x86_64-setup\.exe$/);
  await expect(page.getByRole('link', {name:/portable Windows ZIP/})).toHaveAttribute('href', /windows-x86_64\.zip$/);
  await expect(page.locator('#install-poqi')).toContainText('Start menu');
  await expect(page.locator('#install-poqi')).toContainText('publisher as unknown');
  await page.getByRole('link', {name:'Troubleshoot', exact:true}).click();
  await expect(page).toHaveURL(/#troubleshooting$/);
  await page.locator('details summary').click();
  await expect(page.locator('details')).toHaveAttribute('open', '');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByRole('link', {name:'poqi', exact:true}).click();
  await expect(page).toHaveURL(/\/poqi\/$/);
  await context.close();
});

for (const [choice, file] of [['macos-arm','macos-arm64.tar.gz'], ['macos-intel','macos-x86_64.tar.gz']]) {
  test(`a Mac user must choose its processor before the ${choice} download`, async ({ browser }) => {
    const context = await browser.newContext({
      userAgent:'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/140.0.0.0 Safari/537.36',
      viewport:{width:390,height:844}, reducedMotion:'reduce',
    });
    const page = await context.newPage();
    await page.goto('http://127.0.0.1:4321/poqi/');
    await expect(page.locator('#mac-choice')).toBeVisible();
    await expect(page.locator('#download-link')).not.toBeVisible();
    await page.locator(`[data-mac-choice="${choice}"]`).click();
    await expect(page.locator('#mac-choice')).not.toBeVisible();
    await expect(page.locator(`[data-platform="${choice}"]`)).toBeFocused();
    await expect(page.locator('#download-link')).toHaveAttribute('href', new RegExp(`${file.replaceAll('.', '\\.')}$`));
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await context.close();
  });
}

test('download panel stays stable across platforms with terminal instructions open and closed', async ({ page }) => {
  await page.emulateMedia({reducedMotion:'reduce'});
  for (const width of [360, 390, 570, 768, 1024, 1280, 1440]) {
    await page.setViewportSize({width, height:900});
    await page.goto('/poqi/');
    for (const open of [false, true]) {
      await page.locator('.installer-details').evaluate((el, value) => { el.open = value; }, open);
      let initial;
      let initialLayout;
      for (const platform of ['windows', 'macos-arm', 'macos-intel', 'linux']) {
        await page.locator(`[data-platform="${platform}"]`).click();
        const button = page.locator('#download-link');
        const box = await button.boundingBox();
        expect(box).not.toBeNull();
        initial ||= box;
        expect(box.width).toBe(initial.width);
        expect(box.height).toBe(initial.height);
        const layout = await page.evaluate(() => ['#download-panel', '.installer-details', '.download-secondary', '.first-connection'].map(selector => {
          const rect = document.querySelector(selector).getBoundingClientRect();
          return [rect.width, rect.height, rect.top + scrollY];
        }));
        initialLayout ||= layout;
        expect(layout).toEqual(initialLayout);
        expect(await button.evaluate(el => el.scrollWidth <= el.clientWidth && el.scrollHeight <= el.clientHeight)).toBe(true);
        expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      }
    }
    await page.getByRole('link', {name:'System requirements & setup'}).click();
    await expect(page).toHaveURL(/getting-started\/#requirements$/);
    await expect(page.locator('#requirements')).toBeInViewport();
  }
});
