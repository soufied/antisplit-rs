# antisplit-rs

Merges Android split APK bundles (`.apkm`, `.apks`, `.xapk`, `.apkx`, `.zip`) into one installable APK.

This project is a fork/port of [AntiSplit-M](https://github.com/AbdurazaaqMohammed/AntiSplit-M) to Linux with Rust.

## Requirements

**To build**

- Rust 1.85+ via [rustup](https://rustup.rs)
- GTK 3 and X11/Wayland development libraries (used by the native file dialog)

**To sign the merged APK** (either option works, the app detects whichever is installed; signing is required, since the merged APK must be signed to install)

- **Option A: uber-apk-signer** (recommended, and works without the Android SDK)
  - Java runtime (JRE 8 or newer)
  - `uber-apk-signer.jar`. It also embeds `zipalign` and a debug keystore, so it can sign with no extra setup.
- **Option B: Android build-tools** (`apksigner` + `zipalign`)
  - Looked up in `$ANDROID_HOME`/`$ANDROID_SDK_ROOT`, `~/Android/Sdk`, and `/opt/android-sdk` (`build-tools/<version>/`)

**Optional**

- `zipalign` on `PATH`. If it's missing, uber-apk-signer skips alignment and the app logs a warning. Installing it is recommended, since Android 11+ may reject unaligned APKs.

### Install on Arch / CachyOS

```sh
sudo pacman -S jre-openjdk                  # Java runtime
paru -S uber-apk-signer-bin                 # uber-apk-signer (AUR)
paru -S android-sdk-build-tools             # zipalign / apksigner (AUR)
```

The app finds the jar automatically at `/usr/share/java/uber-apk-signer/uber-apk-signer.jar`. If your package installs it elsewhere, check with `pacman -Ql uber-apk-signer-bin | grep -i jar` and set the path under Advanced settings.

### Install on Debian / Ubuntu

```sh
sudo apt install -y openjdk-17-jre-headless zipalign
```

Download `uber-apk-signer.jar` from the [releases page](https://github.com/patrickfav/uber-apk-signer/releases) and place it in `/usr/share/java/` or set its path under Advanced settings.

## Build

On Debian/Ubuntu install the system libraries first:

```sh
sudo apt install -y build-essential pkg-config libgtk-3-dev libxkbcommon-dev \
  libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev
```

On Arch / CachyOS:

```sh
sudo pacman -S --needed base-devel pkgconf gtk3 libxkbcommon wayland libx11 libxcursor libxrandr libxi mesa
```

Then build the app:

```sh
cd antisplit-rs
cargo run --release       # build and start
cargo build --release     # binary: target/release/antisplit-rs
```

Signing runs after the merge. If no release keystore is configured, the app signs with uber-apk-signer's built-in debug key, which is enough to install the APK for personal use but not to publish it. Set a keystore under Signing to sign with your own key. Leave the key alias empty to use the first key in the keystore, or enter an alias to pick one.

## Usage

1. Choose a bundle (or drag it onto the window).
   Optional: under **Advanced settings → Device**, pick your phone (Google Pixel, Samsung, POCO, Xiaomi, OnePlus, Motorola, Sony, Honor, Huawei, vivo, OPPO and more) to set the architecture and screen density. The values are estimates, so check yours with `adb shell wm density` and adjust them if needed.
2. Select splits, then **Prepare**.
3. **Merge into APK**.

## Settings, shortcuts and recovery

Settings are saved to `$XDG_CONFIG_HOME/antisplit-rs/settings.json` (or `~/.config/antisplit-rs/settings.json`) as sorted, pretty-printed JSON. A legacy `settings.conf` is read once if no JSON file exists yet.

| Shortcut | Action |
| --- | --- |
| Ctrl+O | Choose a bundle |
| Ctrl+P | Prepare the selected bundle |
| Ctrl+M | Merge into APK |
| Ctrl+L | Show or hide the console |
| Esc | Cancel the replace confirmation |

When a step fails, use **Retry** in the error banner to run that step again. Replacing a bundle that already has a prepared selection or merge result asks for confirmation first.

## Credits

- [AntiSplit-M](https://github.com/AbdurazaaqMohammed/AntiSplit-M) by Abdurazaaq Mohammed, the original project this is ported from.
- [APKEditor](https://github.com/REAndroid/APKEditor) by REAndroid.
- [ARSCLib](https://github.com/REAndroid/ARSCLib) by REAndroid, the Android resource-table library.

## License

Licensed under the [Apache License 2.0](LICENSE). This project inherits its license from AntiSplit-M.
