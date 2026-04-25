# Homebrew distribution

This directory holds the Homebrew formula skeleton. The repo itself does
**not** publish to Homebrew — distribution lives in a separate tap repo
that points at the GitHub Release artifacts produced by
`.github/workflows/release.yml`.

## One-time setup (you do this once)

1. **Create a tap repo on GitHub** named `homebrew-tap`:

   ```
   https://github.com/tejasgajare/homebrew-tap
   ```

   The `homebrew-tap` prefix is mandatory — Homebrew looks for that
   exact name when resolving `brew tap tejasgajare/tap`.

2. **Lay it out like this:**

   ```
   homebrew-tap/
   └── Formula/
       └── basitop.rb
   ```

3. Copy `basitop.rb` from this directory into `Formula/basitop.rb` in
   the tap repo.

## Cutting a release

1. Bump `version` in `Cargo.toml` and the `version "x.y.z"` line in
   this `basitop.rb`.
2. Tag the commit and push:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. The `Release` workflow builds two tarballs and attaches them to the
   GitHub Release:
   - `basitop-aarch64-apple-darwin.tar.gz`
   - `basitop-x86_64-apple-darwin.tar.gz`
   (alongside `.sha256` files for each)
4. **Update the SHA256s** in `Formula/basitop.rb` in the tap repo:
   ```bash
   curl -L https://github.com/tejasgajare/basitop/releases/download/v0.1.0/basitop-aarch64-apple-darwin.tar.gz.sha256
   curl -L https://github.com/tejasgajare/basitop/releases/download/v0.1.0/basitop-x86_64-apple-darwin.tar.gz.sha256
   ```
   Paste each value into the corresponding `sha256 "..."` line.
5. Commit the formula update to the tap repo. End users will get the
   new version on their next `brew update && brew upgrade basitop`.

## End-user install

Once the tap repo exists with a populated formula:

```bash
brew tap tejasgajare/tap
brew install basitop
```

Or in one shot:

```bash
brew install tejasgajare/tap/basitop
```
