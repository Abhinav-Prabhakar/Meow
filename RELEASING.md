# Releasing Meow

Meow ships through GitHub Releases, Homebrew, Scoop, winget, and crates.io.
The release itself is built by [cargo-dist](https://axodotdev.github.io/cargo-dist/)
from `.github/workflows/release.yml`.

## Before the tag

1. Bump `version` in `Cargo.toml` and the `meow x.y.z` assertion in `tests/cli.rs`.
2. Check everything:

   ```sh
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   cargo publish --dry-run
   dist plan
   ```

3. Commit and push `main`.

## Cut the release

```sh
git tag vX.Y.Z
git push origin vX.Y.Z
```

The Release workflow builds and uploads:

- macOS Intel and Apple silicon archives;
- Linux x86_64 and arm64 archives;
- a Windows x86_64 zip and MSI;
- the shell and PowerShell installers;
- checksums for every archive;
- the Homebrew formula, committed to `Abhinav-Prabhakar/homebrew-tap`
  using the `HOMEBREW_TAP_TOKEN` secret (a token with `repo` scope).

Watch it with `gh run watch`.

## crates.io

```sh
cargo login        # once per machine
cargo publish
```

The crates.io account must have a verified email address, or the upload is
rejected with a 400 error even when the token is valid.

## Scoop

Update `bucket/meow.json` in `Abhinav-Prabhakar/scoop-meow` with the new
`version`, the Windows zip URL, and its SHA-256. The manifest's `checkver`
and `autoupdate` handle later versions automatically.

## winget

Generate the version, installer, and locale manifests under
`packaging/winget/` from the MSI URL and hash, then open a PR against
`microsoft/winget-pkgs`. The Microsoft CLA must be signed before it can merge.
