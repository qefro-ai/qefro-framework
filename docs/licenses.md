# Licenses

The Qefro Framework is licensed under the MIT License. See `LICENSE` in the repository root.

## Application licenses

Each app declares `license` in `app.toml` (examples use MIT). Packaged `.qefro` files carry that metadata. Do not ship secrets in packages.

## Dependency policy

Rust crates are declared in `Cargo.toml` / `Cargo.lock`. Frontend packages are declared in `frontend/package.json` / `package-lock.json`.

Before a release:

```bash
cargo deny check licenses   # if cargo-deny is installed
# or review Cargo.lock / npm licenses manually
npm --prefix frontend ls
```

Do not add copyleft dependencies that would relicense the framework or applications without an explicit decision.

## Models / AI

Qefro core does not embed model weights. Agent tools call `EntityService`. Any model you attach remains under that model's license; it is not part of the framework distribution.
