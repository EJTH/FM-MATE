# macOS Code Signing Setup

This guide explains how to set up macOS code-signing for FM-MATE to prevent the "app is damaged" warning on modern macOS versions.

## Why Code Signing?

On macOS Ventura and later, Gatekeeper requires all downloaded applications to be signed with a valid code-signing certificate. Without proper code-signing, users encounter:

> "FM-MATE can't be opened because the developer cannot be verified"

or

> "'FM-MATE' is damaged and can't be opened. You should move it to the Trash"

## Prerequisites

- Apple Developer Account ($99/year)
- Xcode Command Line Tools installed
- Access to manage GitHub repository secrets

## Setup Steps

### 1. Create a Code Signing Certificate

1. Go to [Apple Developer Account](https://developer.apple.com/account/)
2. Navigate to **Certificates, Identifiers & Profiles**
3. Create a new **Developer ID Application** certificate:
   - Click **Certificates**
   - Click the **+** button
   - Select **Developer ID Application**
   - Follow the wizard to create the certificate
4. Download the certificate as `.cer` file

### 2. Create a Code Signing Identity

1. Import the downloaded certificate into Keychain
2. Export the certificate as `.p12` file:
   - Open **Keychain Access**
   - Find your certificate
   - Right-click and select **Export**
   - Save as `.p12` with a strong password
   - This is your **signing identity**

### 3. Prepare Credentials

Convert the `.p12` file to base64:

```bash
base64 -i certificate.p12 > certificate.base64
```

You'll need:
- **APPLE_CERTIFICATE**: Base64-encoded `.p12` file
- **APPLE_CERTIFICATE_PASSWORD**: Password for the `.p12` file
- **APPLE_SIGNING_IDENTITY**: Common name of your certificate (e.g., `Developer ID Application: Your Name (XXXXXXXXXX)`)
- **KEYCHAIN_PASSWORD**: Any secure password for the build keychain
- **APPLE_ID**: Your Apple ID email
- **APPLE_PASSWORD**: App-specific password from Apple ID settings
- **APPLE_TEAM_ID**: Your Team ID from Apple Developer Account

### 4. Configure GitHub Secrets

Add the following secrets to your GitHub repository:

1. Go to **Settings** → **Secrets and variables** → **Actions**
2. Add each secret:
   - `APPLE_CERTIFICATE`: Base64-encoded `.p12` certificate
   - `APPLE_CERTIFICATE_PASSWORD`: `.p12` password
   - `APPLE_SIGNING_IDENTITY`: Certificate common name
   - `KEYCHAIN_PASSWORD`: Secure password for build keychain
   - `APPLE_ID`: Your Apple ID email
   - `APPLE_PASSWORD`: App-specific password
   - `APPLE_TEAM_ID`: Your Team ID

### 5. Verify Setup

1. Run the **Build** workflow on GitHub Actions
2. Check the macOS build job logs
3. Download the resulting `.dmg` file
4. On macOS, verify the signature:

```bash
codesign -v /Volumes/fm-mate/fm-mate.app
```

Should output: `valid on disk`

## Troubleshooting

### Certificate not found in keychain

Ensure the certificate was imported correctly:

```bash
security find-identity -v -p codesigning
```

### Signature verification fails

1. Verify the certificate is valid and not expired
2. Ensure the Team ID matches your certificate
3. Check that the signing identity name is exact

### "No such file or directory" in keychain commands

The build keychain setup might have failed. Check:
1. KEYCHAIN_PASSWORD is set correctly
2. Sufficient disk space on runner
3. No special characters in passwords causing escaping issues

## Local Development (Optional)

To test code-signing locally:

```bash
# Set environment variables
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (XXXXXXXXXX)"

# Build with signing
npm run tauri -- build
```

The app will be signed and can be distributed to users without warnings.

## Additional Resources

- [Apple Developer Documentation - Code Signing](https://developer.apple.com/documentation/security/code-signing)
- [Tauri macOS Configuration](https://tauri.app/v1/guides/building/macos/)
- [Developer ID Application Certificates](https://developer.apple.com/developer-id/)
