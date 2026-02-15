# Release Runbook (Windows, GitHub Releases)

This runbook assumes:
- You already validated the app build locally.
- `master` and `origin/master` are in sync.
- GitHub Actions release workflow is configured.

## 0) Preconditions (Before Release Day)

This runbook is written assuming Azure code-signing is already provisioned and active:
- Azure Trusted Signing account exists.
- Certificate profile exists and is `Active`.
- You have the certificate profile metadata available (thumbprint/subject/expiry for audit/reference).
- GitHub Actions authentication to Azure is configured (prefer OIDC).

If these are not true, complete signing setup first (see Section 3).

## 1) Operator Commands (When You Are Ready To Ship)

Run from repo root:

```powershell
# 1. Confirm clean state
git status -sb

# 2. Confirm local master equals origin/master
git fetch origin
git rev-list --left-right --count master...origin/master

# 3. Create release tag (example)
git tag -a v1.0.0 -m "Release v1.0.0"

# 4. Push only the tag (this triggers release workflow)
git push origin v1.0.0

# 5. Watch workflow
gh run list --workflow release-windows.yml --limit 5
```

Expected result:
- Tag push `v*` triggers GitHub Actions.
- Workflow builds/signs artifacts, generates checksums, and publishes GitHub Release.

## 2) What GitHub Actions Must Do

Release workflow trigger:

```yaml
on:
  push:
    tags:
      - "v*"
```

Required pipeline stages:
1. Checkout + Rust toolchain setup.
2. `cargo build --release`.
3. Build installer (`setup.exe`).
4. Sign artifacts (exe + installer) with Azure Trusted Signing + RFC3161 timestamp.
5. Generate `SHA256SUMS.txt`.
6. Publish GitHub Release with:
   - installer
   - standalone exe (optional)
   - `SHA256SUMS.txt`

## 3) Signing Setup (Azure Trusted Signing)

Use Azure Trusted Signing in CI (recommended), not local private key export.

### Azure side
1. Create/confirm Trusted Signing account + certificate profile.
2. Create Entra app/service principal for GitHub Actions.
3. Grant role: `Trusted Signing Certificate Profile Signer` at the correct scope.
4. Configure federated credential (OIDC) limited to this repo and release workflow/tag context.

### GitHub side
Use OIDC with `azure/login` or direct action auth flow.

Repository secrets/vars typically needed:
- `AZURE_TENANT_ID`
- `AZURE_CLIENT_ID`
- `AZURE_SUBSCRIPTION_ID` (if using `azure/login`)
- Signing config values (vars or secrets):
  - endpoint (e.g. `https://<region>.codesigning.azure.net/`)
  - trusted signing account name
  - certificate profile name

Workflow permissions must include:

```yaml
permissions:
  contents: write
  id-token: write
```

Signing action (example):
- `azure/trusted-signing-action@v0`
- Sign `*.exe` in output folders.
- Timestamp: `http://timestamp.acs.microsoft.com` with SHA256.

## 4) Checksum Handling (What/Why)

`SHA256SUMS.txt` is published with release artifacts.

Purpose:
- Lets users verify downloaded file integrity.
- Complements code-signing (checksum != identity; signature handles identity).

User verification command (Windows):

```powershell
certutil -hashfile .\YourSetup.exe SHA256
```

Compare hash with `SHA256SUMS.txt`.

## 5) Release Verification Checklist

After workflow completes:
1. Open GitHub Release for tag `vX.Y.Z`.
2. Confirm assets exist:
   - installer `.exe`
   - app `.exe` (if shipped)
   - `SHA256SUMS.txt`
3. Download installer on clean VM.
4. Verify:
   - signature publisher is correct
   - timestamp present
   - install/uninstall works
   - app starts and basic dictation path works
5. Verify SmartScreen prompt/signature metadata behavior.

## 6) Rollback / Fix Forward

If release is bad:
1. Mark release as pre-release or remove release assets.
2. Create fix on feature branch.
3. Merge to `master`.
4. Tag new patch release (`vX.Y.Z+1`) and push tag.

Avoid reusing/deleting shipped version tags unless absolutely necessary.

## 7) Current Repo Mapping

Current workflow file:
- `.github/workflows/release-windows.yml`

Current installer script:
- `installer/MangoChat.iss`

To productionize fully, ensure this workflow includes signing before checksum/release publish.

## References
- Azure Trusted Signing Action: https://github.com/Azure/trusted-signing-action
- GitHub OIDC with Azure: https://docs.github.com/en/actions/security-for-github-actions/security-hardening-your-deployments/configuring-openid-connect-in-azure
- Azure OIDC login action: https://github.com/Azure/login

