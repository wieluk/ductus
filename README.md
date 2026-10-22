# Ductus

Handwritten notebooks with stylus input, for Linux and Windows (macOS and Android builds too).
Notes are SVG files in plain folders; PDFs get real PDF annotations.

Rust core, Qt 6.10 Quick UI ([cxx-qt](https://github.com/KDAB/cxx-qt)). GPL-3.0-or-later.

## Build

```sh
cargo run -p ductus-qt                          # needs Qt 6.10 (set QMAKE if several are installed)
build-aux/sdk-run.sh cargo build -p ductus-qt   # or inside the KDE Flatpak SDK
```

PDF support loads [pdfium](https://github.com/bblanchon/pdfium-binaries) at runtime:
`build-aux/fetch-pdfium.sh linux-x64 target/pdfium`.

Packages: `build-aux/linux/build-flatpak.sh`, `build-aux/windows/package.ps1`,
`build-aux/macos/build-app.sh`, `build-aux/android/build-apk.sh`.

## Test

```sh
cargo test --workspace --exclude ductus-qt
build-aux/sdk-run.sh build-aux/ui-tests.sh target/sdk-6.10/debug/ductus
```

## Release

Push a version tag: `git tag v0.2.0 && git push origin v0.2.0`. The workflow stamps the
version, builds every platform, signs the checksums and publishes the release.
