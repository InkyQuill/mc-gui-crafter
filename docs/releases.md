# Linux releases

The baseline is GitHub tag `v0.0.1-alpha` at
`7cc113672b75b8ec1f966e4fee13bb814acf5a71`. The existing tag is never rewritten.
Starting with `0.1.1`, releases use plain SemVer without an alpha suffix.
Release Please uses the default strategy: `fix` increments patch, `feat`
increments minor, and breaking changes increment minor while the major is zero.
The transition commit carries `Release-As: 0.1.1` once; subsequent releases are
automatic. Keep that footer when squash-merging the transition PR, because the
default strategy otherwise preserves the previous prerelease suffix.
Version metadata in package.json, Tauri config, Cargo.toml, Cargo.lock and the
release manifest must agree. The workflow synchronizes Cargo.lock in the release
PR before dispatching its checks.

## Installation

Download the matching installer and `SHA256SUMS` from
[GitHub Releases](https://github.com/InkyQuill/mc-gui-crafter/releases).
Verify downloaded files with `sha256sum --check --ignore-missing SHA256SUMS`.
Use your package manager so runtime dependencies are installed:

```sh
sudo apt install ./mc-gui-crafter_*.deb          # Debian/Ubuntu
sudo dnf install ./mc-gui-crafter-*.rpm          # Fedora
sudo pacman -U ./mc-gui-crafter-*.pkg.tar.zst  # Arch
```

Packages target x86_64. The CI build uses Ubuntu 24.04 and system WebKitGTK 4.1;
older distributions are not implicitly supported. Arch pkgver removes the SemVer
hyphen for historical alpha tags (`0.1.0-alpha.1` → `0.1.0alpha.1`).
Plain versions such as `0.1.1` need no conversion.

## Maintainer workflow

1. Merge ordinary changes into `main` using Conventional Commit messages.
2. `Release please` creates/updates a version PR and explicitly dispatches
   `Linux packages` against its branch. No personal access token is needed.
3. Review its changelog/version and wait for successful checks on its current
   commit, then merge the release PR manually.
4. The workflow validates the merged PR identity, creates its tag at that exact
   commit and dispatches `Publish Linux release` on the tag.
5. All three installers are built and checked before a draft GitHub release is
   created. Checksums and `SOURCE_COMMIT` accompany the packages. The draft is
   published only after uploads succeed. Plain versions are normal releases and
   become Latest; historical alpha tags remain prereleases.

Repository Settings → Actions → General must allow GitHub Actions to create
pull requests. The workflow requests explicit write permissions for release
management; package build jobs use read-only repository permissions.

### Bot-created PR approval

Since June 2026, GitHub creates approval-gated `pull_request` runs for PRs
created or updated with `GITHUB_TOKEN`. Approve them in the PR's Checks tab
before merging. If approval expires, GitHub shows a red run without jobs or
build logs: "This workflow run required approval but was not approved before
it expired." This is separate from the explicitly dispatched `Linux packages`
run, which must pass on the release PR's latest commit.

A maintainer can rerun an expired check with `gh run rerun RUN_ID`; inspect the
new attempt before relying on it. Recheck only the final synchronized commit,
not an intermediate commit whose Cargo.lock has not been updated yet.
The workflow token cannot approve its own gated runs. Unattended PR-triggered
checks require a separately configured GitHub App or PAT; the current workflow
uses no such long-lived credential and keeps explicit dispatch for validation.

If tagging/dispatch fails, rerun the failed release-please job. If packaging or
upload fails, rerun `Publish Linux release` on the same tag:

```sh
gh workflow run release.yml --ref v0.1.0-alpha
```

The tag must belong to `main` and match all version files. Existing tags cannot
be redirected by this workflow. A published release's files are never replaced;
a draft can be retried. Artifacts are retained for 30 days. Re-running the entire
build can produce different bytes; publication always uses the artifact from
that same workflow run, without rebuilding in the publish job.

## Local checks

```sh
python3 scripts/release/check-version.py
pnpm install --frozen-lockfile
pnpm verify
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm tauri build --ci --bundles deb,rpm -- --locked
mkdir -p release-assets
cp src-tauri/target/release/bundle/deb/*.deb release-assets/
cp src-tauri/target/release/bundle/rpm/*.rpm release-assets/
scripts/release/package-arch.sh
```

The Arch script uses a disposable container to build with makepkg, install via
pacman, check installed files and dynamic libraries, then uninstall. It does not
open the application on the host display.
