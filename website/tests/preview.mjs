import { preview } from 'astro';

// Playwright owns this foreground server, including shutdown. The Astro CLI can
// otherwise daemonize itself automatically when run from an agent environment.
await preview({server:{host:'127.0.0.1', port:4321}, logLevel:'error'});
