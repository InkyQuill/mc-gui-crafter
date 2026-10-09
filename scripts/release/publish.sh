#!/usr/bin/env bash
set -euo pipefail
python3 scripts/release/check-version.py "$GITHUB_REF_NAME"
cd release-assets
test "$(cat SOURCE_COMMIT)" = "$GITHUB_SHA"
sha256sum --check SHA256SUMS
# Confirm tag identity again immediately before publication.
remote_sha=$(git ls-remote origin "refs/tags/$GITHUB_REF_NAME" | cut -f1)
test "$remote_sha" = "$GITHUB_SHA"
# A published release is immutable to this workflow, including on retries.
releases=$(gh api "repos/$GITHUB_REPOSITORY/releases" --paginate --slurp)
existing=$(jq -c --arg tag "$GITHUB_REF_NAME" '[.[][] | select(.tag_name == $tag)][0] // empty' <<< "$releases")
if [[ -n "$existing" ]]; then
  if [[ $(jq -r .draft <<< "$existing") == false ]]; then
    echo 'Release already published; leaving its assets unchanged.'
    exit 0
  fi
else
  gh release create "$GITHUB_REF_NAME" --verify-tag --target "$GITHUB_SHA" \
    --draft --prerelease --title "MCGUI Crafter $GITHUB_REF_NAME" --generate-notes
fi
gh release upload "$GITHUB_REF_NAME" ./*.deb ./*.rpm ./*.pkg.tar.zst SOURCE_COMMIT SHA256SUMS --clobber
gh release edit "$GITHUB_REF_NAME" --draft=false --prerelease --latest=false
