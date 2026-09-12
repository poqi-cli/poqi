# Cobalt launch design

The selected direction is **A — Cobalt launch**, using the refined layout and detailed application screenshots. Preserve the white/cobalt palette, tightly set headline, offset editor detail, broad terminal stage, and alternating white/cobalt/dark sections.

## Visual decisions

- System Arial/Helvetica sans; no external fonts.
- Cobalt `#2147ee`, ink `#12141b`, white, light stage `#f0f2f7`, dark installation panel `#10131a`.
- Content width 1320px; desktop headline token 148px with responsive sizing, line-height .89 and tracking -.078em.
- Original raster wordmark; no replacement logo or mascot.
- Proportional screenshots with full-size viewing. Native image dialog supports Escape and restores focus to its opener.
- Semantic headings, skip link, visible focus, keyboard-operable platform tabs, reduced-motion handling, and responsive layout.

## Screenshot provenance

The wordmark is the original repository PNG. The six theme screenshots, semantic result and Settings were captured on 2026-09-11 from a real poqi executable through ConPTY and xterm.js, using temporary synthetic demonstration data. They show actual app output, not native Windows Terminal window captures or measured performance.

The detailed editor screenshot was captured on 2026-09-12 using a read-only `SELECT FROM VALUES` service-metrics example. Its executable SHA-256 was `E0A7C203F52296C53DB18A230050902699504C07492FAC6672075BFD4AAB794A`. These screenshots are not claimed to be captures of the public release binary.

The maintained implementation is this Astro project. The earlier standalone HTML/CSS/JS prototype has been removed. Release maintenance, build commands and publication requirements are in [README.md](README.md).
