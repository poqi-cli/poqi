import { version, platforms, downloadUrl } from '../data/release.js';

const tabs = [...document.querySelectorAll('[data-platform]')];

function selectPlatform(id) {
  const config = platforms[id];
  if (!config) return;

  tabs.forEach((tab) => {
    const active = tab.dataset.platform === id;
    tab.setAttribute('aria-selected', String(active));
    tab.tabIndex = active ? 0 : -1;
  });

  document.querySelector('#download-panel').setAttribute('aria-labelledby', `tab-${id}`);
  const link = document.querySelector('#download-link');
  link.href = downloadUrl(config);
  link.setAttribute('aria-label', `Download for ${config.title}`);
  link.replaceChildren(document.createTextNode(`${config.buttonLabel} `));
  const arrow = document.createElement('span');
  arrow.setAttribute('aria-hidden', 'true');
  arrow.textContent = '↓';
  link.append(arrow);
  document.querySelector('#download-meta').textContent = `${version} · ${config.meta}`;
  document.querySelector('#platform-note').textContent = config.note;
  document.querySelector('#command-shell').textContent = config.shell;
  document.querySelector('#install-command').textContent = config.command;
  document.querySelector('#copy-status').textContent = '';
  document.querySelector('#copy-command').textContent = 'Copy';
}

tabs.forEach((tab, index) => {
  tab.addEventListener('click', () => selectPlatform(tab.dataset.platform));
  tab.addEventListener('keydown', (event) => {
    let next;
    if (event.key === 'ArrowRight') next = (index + 1) % tabs.length;
    else if (event.key === 'ArrowLeft') next = (index + tabs.length - 1) % tabs.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = tabs.length - 1;
    else return;

    event.preventDefault();
    selectPlatform(tabs[next].dataset.platform);
    tabs[next].focus();
  });
});

// Both Intel and Apple Silicon browsers can report the same Macintosh user agent.
// Ask for a processor before offering a Mac binary instead of guessing.
const agent = navigator.userAgent;
if (/Linux/i.test(agent) && !/Android/i.test(agent)) selectPlatform('linux');
else if (/Macintosh/i.test(agent)) {
  const chooser = document.querySelector('#mac-choice');
  const tabList = document.querySelector('.platform-tabs');
  const panel = document.querySelector('#download-panel');
  chooser.hidden = false;
  tabList.hidden = true;
  panel.hidden = true;
  const showPlatforms = (platform) => {
    selectPlatform(platform);
    chooser.hidden = true;
    tabList.hidden = false;
    panel.hidden = false;
    tabs.find(tab => tab.dataset.platform === platform)?.focus();
  };
  document.querySelectorAll('[data-mac-choice]').forEach(button => {
    button.addEventListener('click', () => showPlatforms(button.dataset.macChoice));
  });
  document.querySelector('#show-all-platforms').addEventListener('click', () => showPlatforms('windows'));
}

const themeButtons = [...document.querySelectorAll('button[data-theme]')];
const themeTemplates = [...document.querySelectorAll('template[data-theme-image]')];

themeButtons.forEach((button) => button.addEventListener('click', () => {
  const template = themeTemplates.find((item) => item.dataset.themeImage === button.dataset.theme);
  const nextImage = template?.content.querySelector('img');
  if (!nextImage) return;

  const image = document.querySelector('#theme-image');
  ['src', 'srcset', 'sizes', 'width', 'height'].forEach((attribute) => {
    const value = nextImage.getAttribute(attribute);
    if (value) image.setAttribute(attribute, value);
    else image.removeAttribute(attribute);
  });
  image.alt = nextImage.alt;

  const original = template.dataset.original;
  document.querySelector('#main-full').href = original;
  document.querySelector('#main-image-link').href = original;
  themeButtons.forEach((item) => item.setAttribute('aria-pressed', String(item === button)));
}));

document.querySelector('#copy-command').addEventListener('click', async () => {
  const status = document.querySelector('#copy-status');
  const command = document.querySelector('#install-command').textContent;
  try {
    if (navigator.clipboard?.writeText && window.isSecureContext) {
      await navigator.clipboard.writeText(command);
    } else {
      const input = document.createElement('textarea');
      input.value = command;
      input.style.cssText = 'position:fixed;left:-9999px;top:0';
      document.body.append(input);
      input.select();
      let copied;
      try {
        copied = document.execCommand('copy');
      } finally {
        input.remove();
        document.querySelector('#copy-command').focus();
      }
      if (!copied) throw new Error('Clipboard unavailable');
    }
    status.textContent = 'Installation command copied.';
    document.querySelector('#copy-command').textContent = 'Copied';
  } catch {
    status.textContent = 'Copy unavailable. Select the command above and copy it manually.';
  }
});

const viewer = document.querySelector('#image-viewer');
let imageOpener = null;

document.querySelectorAll('a[data-lightbox]').forEach((link) => link.addEventListener('click', (event) => {
  if (event.ctrlKey || event.metaKey || event.shiftKey || event.altKey || !viewer.showModal) return;

  event.preventDefault();
  imageOpener = link;
  const image = link.querySelector('img') || link.closest('figure')?.querySelector('img') || document.querySelector('#theme-image');
  const label = image?.alt || 'poqi application screenshot';
  document.querySelector('#viewer-image').src = link.href;
  document.querySelector('#viewer-image').alt = label;
  document.querySelector('#viewer-title').textContent = label;
  document.querySelector('#viewer-original').href = link.href;
  viewer.showModal();
}));

document.querySelector('#viewer-close').addEventListener('click', () => viewer.close());
viewer.addEventListener('click', (event) => {
  if (event.target !== viewer) return;
  const rect = viewer.getBoundingClientRect();
  const outside = event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom;
  if (outside) viewer.close();
});
viewer.addEventListener('close', () => imageOpener?.focus());
