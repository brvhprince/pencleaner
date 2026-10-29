# Illustrations

`vendor/` holds the animated illustration pack: only the artwork the app uses, since
everything in it ships with the app.

`Illustration.svelte` loads whatever SVGs are present and falls back to a neutral
placeholder, so swapping in your own artwork only means dropping SVGs with the same
names into `vendor/`.

The rest of the pack lives in `unused/`, which git ignores. To use one, move it into
`vendor/` and refer to it by file name (without `.svg`):

```sh
mv ui/src/lib/illustrations/unused/alerts.svg ui/src/lib/illustrations/vendor/ && git add ui/src/lib/illustrations/vendor/alerts.svg
```
