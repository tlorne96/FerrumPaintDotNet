# Installing Ferrum Paint

## Windows 10 / 11

1. Go to the [Releases page](https://github.com/tlorne96/FerrumPaintDotNet/releases) and download `FerrumPaint-<version>-Setup.exe` from the latest release.
2. Run it. The installer isn't code-signed yet, so Windows SmartScreen may say "Windows protected your PC". Click **More info**, then **Run anyway**.
3. Click through the installer. By default it installs for your user only (no admin needed) into `%LOCALAPPDATA%\Programs\Ferrum Paint`. Choose "Install for all users" if you want it in Program Files instead. You can also add a desktop icon and make `.ferrum` files open in Ferrum Paint.

You get a Start Menu entry, "Open with → Ferrum Paint" for images and `.pdn` files, and a normal uninstaller under **Settings → Apps → Installed apps**. Installing a newer version over the top upgrades it.

## Ubuntu 26.04 (and other recent Ubuntu/Debian, 64-bit)

1. Download `ferrum-paint_<version>-1_amd64.deb` from the [Releases page](https://github.com/tlorne96/FerrumPaintDotNet/releases).
2. Install it from a terminal in your Downloads folder:

   ```bash
   sudo apt install ./ferrum-paint_*_amd64.deb
   ```

   Using `apt install ./file.deb` (not `dpkg -i`) pulls in the few libraries it needs automatically.
3. Launch **Ferrum Paint** from the app grid, or run `ferrum-paint`. `.ferrum` files open in it when double-clicked.

To uninstall: `sudo apt remove ferrum-paint`.

## Opening your Paint.NET files

**File → Open** (Ctrl+O) opens `.pdn` files saved by Paint.NET 3.x to 5.x, with all layers, names, visibility, opacity and blend modes kept. Paint.NET's own files are never modified. **Save** (Ctrl+S) asks where to save a layered `.ferrum` copy, and you carry on from that file. **File → Export** gives you a flat PNG or JPEG.

## Building from source

If you'd rather build it yourself, or you're on a platform without a prebuilt package:

1. Install Rust from <https://rustup.rs>.
2. Ubuntu only, install the build dependencies:

   ```bash
   sudo apt install build-essential pkg-config libxkbcommon-dev libgl1-mesa-dev \
       libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
       xdg-desktop-portal xdg-desktop-portal-gtk
   ```
3. Get the code and build it:

   ```bash
   git clone https://github.com/tlorne96/FerrumPaintDotNet.git
   cd FerrumPaintDotNet
   cargo build --release
   ```
4. Install it with a menu entry and file type. Pick one:
   - Make your own installer: `cargo install cargo-deb && cargo deb` (Ubuntu), or `packaging\windows\build-installer.ps1` with Inno Setup 6 (Windows).
   - Use the per-user script installers: `bash packaging/install-linux.sh` (remove with `packaging/uninstall-linux.sh`), or `powershell -ExecutionPolicy Bypass -File packaging\install-windows.ps1` (remove with `packaging\uninstall-windows.ps1`).

If the Windows build fails with `output path is not a writable directory`, your Documents folder is probably protected by Controlled Folder Access or OneDrive. Set `$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\ferrum-paint-target"` and build again.