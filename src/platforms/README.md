# Frontend platform layer

A platform owns everything specific to it under `src/platforms/<id>/`:

```
<id>/
  index.ts        # definePlatform: id, messages, icon, iconStyle, singleChapter
  types.ts        # extra + search/detail item types
  messages.ts     # en + zh-CN (same keys, same placeholders)
  ResultCard.vue
  Detail.vue
  icon.svg|png
  fixtures/wire.json
```

To add a platform:

1. Create the directory with the files above.
2. Add one entry to `platformList` in `src/platforms/index.ts`.
3. Add its branch to `PlatformCard.vue` and `PlatformDetail.vue`.
4. Add its Rust counterpart under `src-tauri/src/platforms/` and register it in
   `registry.rs`.

Everything else (picker, router pattern, list labels, home/search pages,
history/favorites/downloads screens) reads the registry and needs no edit.

Platform copy uses `usePlatformText(messages)` from `messages.ts`; shared UI copy
stays in `src/i18n/messages/`. `messages.test.ts` verifies that every platform
has the same keys and placeholders in both locales.

## Icons

These are the real platform icons, not generic drawings. Prefer official assets
(raster or vector). When a platform only ships a tiny favicon, a pixel-accurate
SVG redraw of that favicon is acceptable; do not invent unrelated artwork.

| File | Source | Notes |
| --- | --- | --- |
| `manhuagui/icon.svg` | <https://www.manhuagui.com/favicon.ico> | 16×16 favicon redrawn as a pixel-accurate SVG (8 colors, 60 runs) |
| `jm/icon.png` | <https://syimg.3dmgame.com/uploadimg/ico/2026/0730/1785403601108808.jpg> | Outer white border removed and cropped to a transparent circle |
| `nhentai/icon.svg` | <https://nhentai.net/logo.svg> | Official vector logo; its white strokes need the `dark-chip` style in `PlatformPicker` |
| `copymanga/icon.png` | <https://s3.mangafunb.fun/static/websitefree/jpg/allIcon.png> | Cropped 68×67 from the site sprite at `-34px -590px` (the header logo) |
| `ehentai/icon.png` | <https://e-hentai.org/favicon.ico> | 16×16 favicon upscaled 4× with nearest-neighbor to keep the pixel edges crisp |
| `pixiv/icon.png` | <https://www.pixiv.net/favicon.ico> | 48×48 favicon upscaled 2× with LANCZOS |
| `manwa/icon.png` | <https://manwa.me/favicon.ico> | 48×48 favicon used as-is (RGBA) |

Prepared on 2026-10-25; the copymanga sprite was cropped on 2026-10-28; the
E-Hentai favicon was converted on 2026-10-28; the pixiv favicon was converted
on 2026-10-28; the manwa favicon was installed on 2026-10-28.
Re-download and overwrite these files if a platform changes its branding. The
icons remain the property of their respective sites and are used only to
identify the platform.
