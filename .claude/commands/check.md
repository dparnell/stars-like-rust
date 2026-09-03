---
description: Run the full local CI gate (fmt, clippy, build, test) like .github/workflows/ci.yml
---

Run the same gate CI runs, in order, and stop at the first failure:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --all-targets
cargo test --workspace
```

If `cargo fmt --check` fails, run `cargo fmt --all` and show what changed.
Report failures with the actual compiler/test output — do not summarise a
failure as a pass. If everything is green, say so in one line.
