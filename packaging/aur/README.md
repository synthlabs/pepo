# AUR release setup

The release workflow publishes the native Arch artifact to GitHub Releases, then updates
[`pepo-bin`](https://aur.archlinux.org/packages/pepo-bin) with the artifact's exact URL and SHA-256
checksum.

Create a dedicated Ed25519 key for the workflow:

```bash
ssh-keygen -t ed25519 -f ./pepo-aur -C 'pepo AUR release automation'
cat ./pepo-aur.pub
```

Add the public key to the SSH Public Key field in your AUR account, then store the private key in
the Pepo repository:

```bash
gh secret set AUR_SSH_PRIVATE_KEY --repo synthlabs/pepo < ./pepo-aur
```

The workflow scans the AUR Ed25519 host key and requires the published fingerprint
`SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4` before adding it to `known_hosts`.

Package versions use the Tauri application version as `pkgver` and the GitHub Actions run number as
`pkgrel`. The publication script refuses equal or older versions, retries once after a
non-fast-forward update, and does not add a detached GPG signature.

The reusable AUR publisher lives in `utils/packaging/tauri/aur/`; this directory keeps only Pepo's
`PKGBUILD.in` descriptor and release-setup documentation.
