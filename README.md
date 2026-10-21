# AntiSplit-Rust

## About

Merges split APK bundles (`.apks`, `.xapk`, `.apkm`, `.apkx`) into one installable `.apk`. A Linux port of [AntiSplit-M](https://github.com/AbdurazaaqMohammed/AntiSplit-M), written in Rust.

## Requirements

- Rust 1.85+ and GTK 3 development libraries (to build)
- Java 8+ and `uber-apk-signer.jar` (recommended), or Android build-tools (`apksigner`, `zipalign`) for signing
- `zipalign` on `PATH` (recommended for Android 11+)

Arch / CachyOS:

```sh
sudo pacman -S jre-openjdk gtk3 base-devel
paru -S uber-apk-signer-bin android-sdk-build-tools
```

Debian / Ubuntu:

```sh
sudo apt install -y openjdk-17-jre-headless zipalign build-essential pkg-config libgtk-3-dev
```

## Build

```sh
cargo build --release   # binary: target/release/antisplit-rs
```

## Usage

1. Open a bundle (Ctrl+O, or drag it onto the window).
2. Select splits, then **Prepare** (Ctrl+P).
3. **Merge into APK** (Ctrl+M).

## Credits

- [AntiSplit-M](https://github.com/AbdurazaaqMohammed/AntiSplit-M) by Abdurazaaq Mohammed
- [APKEditor](https://github.com/REAndroid/APKEditor) and [ARSCLib](https://github.com/REAndroid/ARSCLib) by REAndroid

## License

Licensed under the [Apache License 2.0](LICENSE). This project inherits its license from AntiSplit-M.
