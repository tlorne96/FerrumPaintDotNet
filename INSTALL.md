# Installing Ferrum Paint

Ferrum Paint is built from source with Rust. There are no prebuilt downloads yet, but the scripts below do everything for you, including the Rust build, a Start Menu or app-launcher entry, icons, and double-click opening of `.ferrum` files. Nothing is installed system-wide and no administrator rights are needed.

The first build downloads the dependencies and compiles them, which takes about 3–10 minutes depending on your machine. Later builds are much faster.

- [Windows 10 / 11](#windows-10--11)
- [Ubuntu 26.04 (and other recent Ubuntu/Debian releases)](#ubuntu-2604)
- [Updating](#updating)
- [Uninstalling](#uninstalling)
- [Troubleshooting](#troubleshooting)

---

## Windows 10 / 11

### 1. Install the tools (one-off)

You need **Git**, **Rust** and the **Microsoft C++ build tools** that Rust uses on Windows.

Open **PowerShell** (press Start, type `powershell`, press Enter) and run:

```powershell
winget install --id Git.Git -e
winget install --id Rustlang.Rustup -e
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

If you don't have `winget`, download the installers instead:

- Git: <https://git-scm.com/download/win>
- Rust: <https://rustup.rs> (run `rustup-init.exe` and accept the defaults; it offers to install the C++ build tools if they are missing)

**Close PowerShell and open a new one** so it picks up the new tools. Check that they work:

```powershell
git --version
cargo --version
```

### 2. Download the source

```powershell
cd $HOME
git clone https://github.com/tlorne96/FerrumPaintDotNet.git
cd FerrumPaintDotNet
```

(No Git? On the GitHub page click **Code → Download ZIP**, extract it, and `cd` into the extracted folder instead.)

### 3. Build and install

```powershell
powershell -ExecutionPolicy Bypass -File packaging\install-windows.ps1
```

Add `-DesktopShortcut` to the end if you also want a desktop icon.

The script:

1. builds a release version of Ferrum Paint,
2. copies it to `%LOCALAPPDATA%\Programs\Ferrum Paint\`,
3. adds **Ferrum Paint** to the Start Menu,
4. makes `.ferrum` files open in Ferrum Paint when you double-click them (current user only; skip this with `-NoFileAssociation`).

### 4. Run it

Press Start and type **Ferrum Paint**. You can also open an image from the command line:

```powershell
& "$env:LOCALAPPDATA\Programs\Ferrum Paint\ferrum-paint.exe" picture.png
```

To pin it to the taskbar, right-click the running app's taskbar icon and choose **Pin to taskbar**.

### Building without the installer (portable)

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\ferrum-paint-target"
cargo build --release
& "$env:CARGO_TARGET_DIR\release\ferrum-paint.exe"
```

`ferrum-paint.exe` is a single self-contained file. You can copy it anywhere, such as a USB stick, and run it from there. Settings are stored in `%APPDATA%\Ferrum Paint\settings.ini`.

---

## Ubuntu 26.04

The same steps work on Ubuntu 24.04 and later, and on Debian-based distributions in general. Ferrum Paint runs on both Wayland and X11.

### 1. Install the build dependencies (one-off)

Open a terminal (Ctrl+Alt+T) and run:

```bash
sudo apt update
sudo apt install -y git curl build-essential pkg-config \
    libxkbcommon-dev libgl1-mesa-dev libwayland-dev \
    libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
    xdg-desktop-portal xdg-desktop-portal-gtk
```

The `xdg-desktop-portal` packages provide the Open/Save dialogs. They're normally already installed on Ubuntu Desktop.

### 2. Install Rust (one-off)

Use rustup rather than the `apt` version of Rust, which can be too old:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
cargo --version
```

### 3. Download, build and install

```bash
cd ~
git clone https://github.com/tlorne96/FerrumPaintDotNet.git
cd FerrumPaintDotNet
bash packaging/install-linux.sh
```

The script builds a release version and installs, for your user only:

- the program, to `~/.local/bin/ferrum-paint`
- an app-launcher entry with icons (search for **Ferrum Paint** in Activities)
- the `.ferrum` file type, so double-clicking a `.ferrum` file opens it in Ferrum Paint.

### 4. Run it

Search for **Ferrum Paint** in Activities, or run `ferrum-paint` in a terminal. If the terminal says `command not found`, `~/.local/bin` isn't on your `PATH` yet. Log out and back in, or run:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc
```

Settings are stored in `~/.config/ferrum-paint/settings.ini`.

---

## Updating

Pull the latest source and run the installer again. It replaces the old version and keeps your settings.

```powershell
# Windows
cd $HOME\FerrumPaintDotNet
git pull
powershell -ExecutionPolicy Bypass -File packaging\install-windows.ps1
```

```bash
# Ubuntu
cd ~/FerrumPaintDotNet
git pull
bash packaging/install-linux.sh
```

## Uninstalling

```powershell
# Windows (add -RemoveSettings to delete settings too)
powershell -ExecutionPolicy Bypass -File packaging\uninstall-windows.ps1
```

```bash
# Ubuntu (add --remove-settings to delete settings too)
bash packaging/uninstall-linux.sh
```

Your images are never touched. To free the disk space used by the build, also delete `%LOCALAPPDATA%\ferrum-paint-target` on Windows, or the `target` folder in the source folder on Ubuntu.

---

## Troubleshooting

**`cargo` / `git` is not recognised (Windows).** Open a *new* PowerShell window after installing them. If it still fails, sign out and back in.

**`linker 'link.exe' not found` (Windows).** The Microsoft C++ build tools are missing. Run the `Microsoft.VisualStudio.2022.BuildTools` command from step 1, or run the Visual Studio Installer and add **Desktop development with C++**.

**`output path is not a writable directory` (Windows).** This happens when the source folder is under Documents or OneDrive with Controlled Folder Access turned on. The install script already avoids it by building into `%LOCALAPPDATA%\ferrum-paint-target`. For manual builds, set `CARGO_TARGET_DIR` as shown in [Building without the installer](#building-without-the-installer-portable).

**"Running scripts is disabled on this system" (Windows).** Use the exact command shown, which includes `-ExecutionPolicy Bypass`. It only applies to that one run.

**The Open/Save dialog doesn't appear (Ubuntu).** Install the portal packages, then log out and back in:
`sudo apt install xdg-desktop-portal xdg-desktop-portal-gtk`.

**The window is blank or fails to start (Ubuntu, VMs, or old GPUs).** Ferrum Paint needs OpenGL 3.0 or newer. On virtual machines, enable 3D acceleration. Software rendering also works: `LIBGL_ALWAYS_SOFTWARE=1 ferrum-paint`.

**`error: failed to run custom build command` mentioning `xkbcommon`, `wayland` or `x11` (Ubuntu).** One of the `-dev` packages from step 1 is missing. Run the `apt install` line again.

**Double-clicking `.ferrum` files doesn't open Ferrum Paint.** Windows: run `& "$env:LOCALAPPDATA\Programs\Ferrum Paint\ferrum-paint.exe" --register-file-type`, then right-click a `.ferrum` file → **Open with → Choose another app → Ferrum Paint → Always**. Ubuntu: run `xdg-mime default ferrum-paint.desktop application/x-ferrum-paint`.

Still stuck? Open an issue on [GitHub](https://github.com/tlorne96/FerrumPaintDotNet/issues). Include your OS version and the full error text.
