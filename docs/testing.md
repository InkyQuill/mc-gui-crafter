# Verification

Run the frontend contract checks and native backend tests:

```sh
pnpm verify
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

## Editor interaction regressions

`scripts/check-editor-ui.cjs` exercises the actual Svelte components in headless
Chromium against Vite's browser mock. It covers both pointer-drag directions for
New Project, Export, Preferences, Shortcuts and Pixel Editor; texture-pack focus
entry/trapping/restoration and Escape; empty slot-grid inputs; context-menu focus
for a single element and clipping at viewport edges. It is not a native Tauri
persistence or Minecraft runtime test.

Install optional browser-test tooling outside the project's dependency tree:

```sh
npm install --prefix /tmp/mcgui-browser-tests playwright
/tmp/mcgui-browser-tests/node_modules/.bin/playwright install chromium
pnpm dev --port 49325 --strictPort
```

In a second terminal:

```sh
NODE_PATH=/tmp/mcgui-browser-tests/node_modules node scripts/check-editor-ui.cjs
```

`MCGUI_TEST_URL` overrides the default `http://127.0.0.1:49325`.
`PLAYWRIGHT_CHROMIUM_EXECUTABLE` optionally selects an existing compatible browser.
The script creates a fresh browser context and never opens the host desktop app.
Archive persistence and malformed center data are covered by Rust ZIP round-trip
tests; integer bounds and history preservation are covered by Vitest.
