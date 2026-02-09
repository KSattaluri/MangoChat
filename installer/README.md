# Windows Packaging (Jarvis)

This project uses Inno Setup to produce a per-user Windows installer.

## Local build

1. Install Inno Setup 6.
2. From repo root:

```powershell
.\scripts\build-installer.ps1
```

Output:
- `dist\Jarvis-Setup-<version>.exe`

Notes:
- Install location is `%LOCALAPPDATA%\Programs\Jarvis` (no admin required).
- Uninstall removes app binaries/shortcuts, and keeps user data.

## GitHub Releases automation

A workflow is provided at:
- `.github/workflows/release-windows.yml`

Trigger:
- Push a tag like `v0.1.0`.

What it does:
1. Builds `jarvis.exe` in release mode.
2. Builds installer with Inno Setup.
3. Publishes installer and `SHA256SUMS.txt` to the GitHub Release.

