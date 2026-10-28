# Backend platform layer

All platform-specific request building and parsing lives here.

```
platforms/
  mod.rs        # ImageProfile / Catalog traits, RequestSpec, shared helpers
  registry.rs   # single registration list + lookup / known-platform checks
  model.rs      # cross-platform wire core (chapters, reader pages, ...)
  <id>.rs       # one implementation per platform
```

To add a platform:

1. Add `src-tauri/src/platforms/<id>.rs` implementing `ImageProfile`
   (required) and `Catalog` (search/detail/reader).
2. Add `mod <id>;` and one `Registration` entry in
   `platforms/mod.rs` / `platforms/registry.rs`.
3. Keep upstream parser fixtures next to the implementation and add a
   `fixtures/wire.json` under the frontend platform directory; the Rust test
   and `src/api/contract.test.ts` both read that file.

Override only the capability hooks that differ from the defaults:

- `single_chapter()` for one implicit chapter (nhentai, E-Hentai, pixiv)
- `normalize_work_id()` for non-numeric ids (JM prefix, copymanga path words,
  E-Hentai `gid-token`)
- `normalize_chapter_id()` for non-numeric chapter ids (copymanga UUIDs)
- `comic_detail()` when detail needs more than one request (copymanga fetches
  one chapter list per site group)
- `reader_chapter()` when the page list spans several documents (E-Hentai
  reads paginated gallery pages, pixiv fetches metadata and pages together)
- `resolve_image_url()` when a stable page URL must be resolved to a
  short-lived image URL (E-Hentai `/s/...` pages)
- `decode_key()` / `preprocess()` for bytes that differ from the download (JM)
- `decode_key()` / `decode_image()` when the image host encrypts bytes at rest
  (manwa's AES-128-CBC page images); `decode_image()` only runs when the magic
  bytes are not a recognisable image
- `cache_key_url()` / `image_fallbacks()` / `resolve_image_url()` /
  `configure()` for CDN quirks (pixiv prefers its `i.pixiv.re` proxy and
  normalizes the cache key to `i.pximg.net`)
- `RequestSpec::json_post()` for JSON APIs that require POST (E-Hentai `gdata`)

The shared `image/` pipeline only calls these traits; it must not learn a new
platform name.
