# Screenshots

Images used by the top-level `README.md` and `README.zh-CN.md` live in this
folder. They were captured from a real build of the app (`pnpm tauri build`,
Windows, 1200×800 window, English locale).

## Files

| File | Content |
| --- | --- |
| `home.png` | Home screen: search bar, platform button and the four shortcuts |
| `platforms.png` | The platform picker open, showing all seven sources |
| `search.png` | Search results grid (pixiv, `風景`) |
| `detail.png` | Comic detail page with metadata, description and tags |
| `settings.png` | Settings page: theme, language, reader, network, logs, cache |
| `dark.png` | Home screen in the dark theme |

## Re-capturing

The screenshots show live upstream data, so a few rules apply when replacing
them:

- Capture from a real run of the app; do not mock up the UI.
- Use a 1200×800 window so the layout matches what most readers see.
- Prefer a source with presentable content. Never commit a capture that shows
  adult or otherwise unpublishable material, and never commit one that exposes
  personal data — favorites, history, downloads, a proxy address or log output
  all come from the machine that produced it.
- Run against a clean data directory. Favorites, history and downloads are real
  user data under `%APPDATA%\com.meik.comics-reader`; move those files aside
  (and back) rather than publishing a capture of them.
- Keep each PNG under ~500 KB.

## Not yet captured

`reader.png` (the reader overlay on an open page) is missing. The reader needs
page images from a source that both loads in the capture environment and has
publishable content, which was not available when this set was produced. Add it
here and in both READMEs once such a source is available.
