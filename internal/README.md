# Internal Features

This repository is checked out at `internal/` in Pepo and Scrybe.

- `main` contains the private implementation used for local development.
- `starter` contains the OSS-safe implementation copied into both public repositories.

The branch is a superset: Pepo consumes the Rust crate while Scrybe consumes `rust/mod.rs`. Both
apps share the frontend entrypoint, generated binding, command list, and build-info plugin contract.

## Run

From either app root:

```sh
make dev-internal
```

Without `ENABLE_INTERNAL=1`, each app uses its tracked public no-op hooks.

## Update the public starter

Commit the OSS-safe interface change on `starter`, then synchronize it from each app checkout:

```sh
cd internal
git switch starter
git pull --ff-only
sh scripts/starter-sync.sh stage pepo ..
```

Use `scrybe` instead of `pepo` in the Scrybe checkout. The script stages only the approved manifest
and writes `.starter-revision`; it never commits or pushes. Review the parent repository's staged
diff, run `sh internal/verify.sh`, and commit from the parent repository.

Switch the nested checkout back to `main` for private development.

## Verify

```sh
sh internal/verify.sh
```

The verification runs public and internal-enabled frontend and Rust checks. Private CI repeats the
same coverage against clean Pepo and Scrybe checkouts.
