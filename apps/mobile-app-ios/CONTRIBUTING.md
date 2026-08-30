# Contributing

## Local Setup

1. Install Xcode 26.x.
2. Install the command line tools listed in `Tools/versions.env`.
3. From the Extrittio repository root, run `cargo xtask ios bootstrap`.

## Development Loop

Before opening a pull request, run:

```bash
cargo xtask ios build
cargo xtask ios test
cargo xtask ios lint
cargo xtask ios format --check
cargo xtask ios module-check
```

Regenerate the Xcode project with `cargo xtask ios generate` after editing `project.yml`, changing build settings, or adding/removing Swift files.

## Code Organization

- Keep domain entities, repository protocols, and use cases in `Extrittio/Domain`.
- Keep transport, persistence, auth storage, and repository implementations in `Extrittio/Data`.
- Keep SwiftUI views, view models, shared UI, and design system code in `Extrittio/Presentation`.
- Keep app composition and object wiring in `Extrittio/DI`.

Do not add Data or Presentation dependencies to Domain. Run `cargo xtask ios module-check` when touching boundaries.

## Pull Requests

- Keep changes scoped and reviewable.
- Include tests for domain, decoding, cache, and view-model behavior when logic changes.
- Update the relevant project documentation when setup, security, or operational behavior changes.
- Do not commit generated `Extrittio.xcodeproj`, build artifacts, secrets, certificates, or personal IDE settings.
