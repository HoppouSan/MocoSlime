# Contributing to Mocoslime

Thanks for helping improve Mocoslime. Issues and pull requests are welcome.

## Before opening an issue

- Search existing issues for the same problem.
- Include Windows version, Mocoslime version, tracker firmware, Bluetooth adapter, and SlimeVR Server version when relevant.
- Attach a redacted diagnostics export from the Logs screen. Review it before sharing; it can contain hardware and network details if you explicitly include private information.
- Describe the expected and observed behavior and the shortest steps that reproduce it.

## Pull requests

- Keep changes focused and explain user-facing behavior in the PR description.
- Preserve the existing Rust/Flutter boundaries and serialized config compatibility.
- Do not add a filter, drift correction, or pose offset without repeatable measurements from real Mocopi hardware and an explanation of added latency.
- For UI changes, keep English source strings and update German, Japanese, and Simplified Chinese translations.
- Include or update regression coverage for protocol, config migration, tracking math, and state changes where appropriate.
- Run `cargo fmt --all -- --check`, `cargo test --workspace`, and from `flutter/` run `dart format --output=none --set-exit-if-changed lib test`, `flutter analyze`, and `flutter test` before requesting review.

## License

By submitting a contribution, you agree that it is provided under the MIT License in `LICENSE`, consistent with the project license. Keep third-party attribution and license notices intact.
