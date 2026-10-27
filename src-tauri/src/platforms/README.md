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

- `single_chapter()` for one implicit chapter (nhentai)
- `normalize_work_id()` for non-numeric ids (JM prefix)
- `decode_key()` / `preprocess()` for bytes that differ from the download (JM)
- `cache_key_url()` / `image_fallbacks()` / `configure()` for CDN quirks

The shared `image/` pipeline only calls these traits; it must not learn a new
platform name.
