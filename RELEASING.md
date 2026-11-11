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

The manifests under `packaging/winget/` install the portable Windows zip, not
the MSI, so `meow` lands on `PATH` as a command:

- `AbhinavPrabhakar.Meow.yaml` — package identifier and version;
- `AbhinavPrabhakar.Meow.installer.yaml` — the x64 zip URL and SHA-256, with
  `NestedInstallerType: portable` and `PortableCommandAlias: meow`;
- `AbhinavPrabhakar.Meow.locale.en-US.yaml` — the store listing.

For a new version, copy the directory to `manifests/a/AbhinavPrabhakar/Meow/X.Y.Z/`
in a fork of `microsoft/winget-pkgs`, bump the version, URL, hash, and release
date, then open a PR. The Microsoft CLA has to be signed once per account by
commenting `@microsoft-github-policy-service agree` on the first PR.
