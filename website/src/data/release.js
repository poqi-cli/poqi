export const repo = 'https://github.com/poqi-cli/poqi';
export const version = 'v1.0.2';
const shellUnix = "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/poqi-cli/poqi/releases/latest/download/install.sh | sh";
export const platforms = {
  windows: {
    title: 'Windows',
    buttonLabel: 'Download for Windows',
    file: 'windows-x86_64-setup.exe',
    meta: 'EXE · 8.8 MB',
    note: 'Setup for 64-bit Intel and AMD PCs. Open the wizard, complete the steps, then launch poqi from the Start menu. The setup is unsigned, so Windows identifies its publisher as unknown and may show a warning before opening it.',
    shell: 'Run in PowerShell. The installer adds poqi to your user PATH.',
    command: 'powershell -ExecutionPolicy Bypass -c "irm https://github.com/poqi-cli/poqi/releases/latest/download/install.ps1 | iex"',
  },
  'macos-arm': {
    title: 'macOS Apple Silicon',
    buttonLabel: 'Download for macOS',
    file: 'macos-arm64.tar.gz',
    meta: 'TAR.GZ · 9.3 MB',
    note: 'For Macs with an Apple M-series chip. Extract the download and run ./poqi in your terminal.',
    shell: 'Run in your terminal. The installer detects your architecture and sets up a user-local command.',
    command: shellUnix,
  },
  'macos-intel': {
    title: 'macOS Intel',
    buttonLabel: 'Download for macOS',
    file: 'macos-x86_64.tar.gz',
    meta: 'TAR.GZ · 9.6 MB',
    note: 'For Macs with an Intel processor. Extract the download and run ./poqi in your terminal.',
    shell: 'Run in your terminal. The installer detects your architecture and sets up a user-local command.',
    command: shellUnix,
  },
  linux: {
    title: 'Linux x64',
    buttonLabel: 'Download for Linux',
    file: 'linux-x86_64.tar.gz',
    meta: 'TAR.GZ · 11.6 MB',
    note: 'For Ubuntu 24.04 on 64-bit Intel and AMD PCs. See system requirements for other Linux distributions.',
    shell: 'Run in your shell. The installer sets up poqi in a user-local bin directory.',
    command: shellUnix,
  },
};
export const windowsPortable = {
  title: 'Windows portable ZIP',
  file: 'windows-x86_64.zip',
  meta: 'ZIP · 9.1 MB',
  note: 'No setup wizard. Extract the ZIP and run .\\poqi.exe from its folder.',
};
export const releaseUrl = `${repo}/releases/tag/${version}`;
export const licenseUrl = `${repo}/blob/${version}/LICENSE`;
export const downloadUrl = (platform) => `${repo}/releases/download/${version}/poqi-${version}-${platform.file}`;
