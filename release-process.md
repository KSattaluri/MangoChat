# Mango Chat Release Process

This document explains how Mango Chat gets built, packaged into an installer,
published on GitHub, and downloaded by users.

---

## Overview

The release pipeline has three stages:

```
Source code  -->  Compiled binary  -->  Windows installer  -->  GitHub Release
  (Rust)         (mangochat.exe)       (MangoChat-Setup-x.y.z.exe)   (public download page)
```

Everything is already wired up. The workflow file, build script, and installer
config all exist in the repo today.

---

## Stage 1: Build the Binary

Rust compiles all your source code into a single `mangochat.exe`.

- **Command:** `cargo build --release`
- **Output:** `target/release/mangochat.exe`
- **What happens:** Cargo resolves all dependencies (egui, tokio, cpal, etc.),
  compiles them along with your `src/` code, and produces one standalone `.exe`.
- Release mode enables optimizations and strips the console window.

---

## Stage 2: Package the Installer

[Inno Setup 6](https://jrsoftware.org/isinfo.php) takes the compiled `.exe`
and wraps it in a traditional Windows installer.

- **Config file:** `installer/MangoChat.iss`
- **Build script:** `scripts/build-installer.ps1`
- **Output:** `dist/MangoChat-Setup-<version>.exe`

What the installer does for end users:

| Step | Detail |
|------|--------|
| Install location | `%LOCALAPPDATA%\Programs\MangoChat` (per-user, no admin needed) |
| Start Menu shortcut | Created automatically |
| Desktop shortcut | Optional (user chooses during install) |
| Post-install | Launches the app immediately |
| Uninstall | Clean removal via Windows "Add/Remove Programs" |

### Running locally (for testing)

```powershell
# Option A: Let script auto-detect version from Cargo.toml
.\scripts\build-installer.ps1

# Option B: Specify version and build name explicitly
.\scripts\build-installer.ps1 -Version "0.2.0" -BuildName "beta1"
```

Prerequisite: [Inno Setup 6](https://jrsoftware.org/isdl.php) must be
installed on your machine.

---

## Stage 3: Publish on GitHub

This is the part that "establishes reputation." GitHub Releases gives you:

- A permanent, public download page per version
- Auto-generated changelog (list of commits since last release)
- SHA256 checksums so users can verify file integrity
- Download counts visible on the release page
- A history of all past versions

### How it works

A GitHub Actions workflow (`.github/workflows/release-windows.yml`) runs
**automatically** when you push a version tag. You do not need to upload
anything manually.

The workflow runs on GitHub's servers (a Windows machine) and does:

1. Checks out your code
2. Installs Rust
3. Runs `cargo build --release`
4. Installs Inno Setup
5. Reads the version from `Cargo.toml`
6. Builds the installer
7. Generates `SHA256SUMS.txt` (checksum file)
8. Creates a GitHub Release with the installer + checksums attached

### What you do to trigger a release

```bash
# 1. Make sure all your changes are committed on main
git checkout main

# 2. Update the version in Cargo.toml (e.g., 0.1.0 -> 0.2.0)
#    Edit Cargo.toml, commit the change

# 3. Create a version tag
git tag v0.2.0

# 4. Push the tag to GitHub
git push origin v0.2.0
```

That's it. The tag push triggers the workflow. A few minutes later, a release
page appears at:

```
https://github.com/<your-username>/diction-wt-assemblyai/releases/tag/v0.2.0
```

### What the release page looks like to users

```
Mango Chat v0.2.0
─────────────────────────────
[auto-generated changelog: list of commits since v0.1.0]

Assets:
  MangoChat-Setup-0.2.0.exe    (12 MB)   <-- users click this to download
  SHA256SUMS.txt             (1 KB)    <-- for verification
  Source code (zip)                     <-- auto-included by GitHub
  Source code (tar.gz)                  <-- auto-included by GitHub
```

Users click the `.exe`, run the installer, done.

---

## How people find and download it

- **Releases page:** `https://github.com/<you>/diction-wt-assemblyai/releases`
  shows all versions. The latest is at the top.
- **Latest shortcut:** `https://github.com/<you>/diction-wt-assemblyai/releases/latest`
  always points to the newest release.
- **README link:** You can add a "Download" badge or link in your README
  pointing to the latest release.
- **Direct link to installer:** Each release asset has a permanent URL like
  `https://github.com/<you>/diction-wt-assemblyai/releases/download/v0.2.0/MangoChat-Setup-0.2.0.exe`

---

## Why this builds reputation

GitHub Releases is the standard way open-source projects distribute software.
Having a clean release history signals:

- **Active maintenance** -- users see regular version bumps
- **Traceability** -- every release links back to the exact source code
- **Integrity** -- SHA256 checksums let users verify downloads aren't tampered with
- **Professionalism** -- proper versioning (v0.1.0, v0.2.0, ...) and changelogs

---

## Quick reference: release checklist

```
[ ] Code changes committed and pushed to main
[ ] Version bumped in Cargo.toml
[ ] Version bump committed
[ ] Tag created:  git tag v<version>
[ ] Tag pushed:   git push origin v<version>
[ ] Wait for GitHub Actions to finish (~3-5 min)
[ ] Verify release page has the installer attached
[ ] (Optional) Edit release notes on GitHub if you want to add context
```

---

## What is NOT set up yet (from todo.md item #10)

The current pipeline handles **creating releases**. The remaining parts from
the todo item that are not yet implemented:

| Feature | Description | Status |
|---------|-------------|--------|
| Create releases | Tag-triggered build + publish | Already working |
| Update indicator in-app | App checks GitHub for newer version and shows a badge | Not built |
| Auto-update | App downloads and installs updates itself | Not built |
| Release notes in-app | App shows what changed in the new version | Not built |

These in-app features are enhancements you can decide on later. The core
release pipeline is functional today.

