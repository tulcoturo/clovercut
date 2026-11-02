# Install Clovercut on macOS

Download the DMG for your Mac from the [official releases](https://github.com/tulcoturo/clovercut/releases): `arm64` for Apple Silicon, or `x86_64` for Intel. Open the DMG, drag Clovercut into Applications, then launch it from there.

## First launch and Gatekeeper

The current 0.2.2 packages are not Apple-notarized. Their ad-hoc signature does not identify a verified Apple developer, so macOS can display “Apple cannot check it for malicious software”. This is separate from the app's macOS version compatibility.

If you downloaded the package from the official release and choose to trust it, follow [Apple's instructions for opening an unnotarized app](https://support.apple.com/en-us/102445):

1. Try opening Clovercut once.
2. On Monterey, open **System Preferences → Security & Privacy → General** and choose **Open Anyway** for Clovercut. On newer macOS versions, use **System Settings → Privacy & Security**.
3. Confirm **Open** when prompted. The exception applies to this application.

On Monterey, Control-clicking Clovercut in Applications and choosing **Open** can also offer the confirmation. You do not need to disable Gatekeeper globally or recursively remove extended attributes.

If macOS instead reports that the app will damage your computer or that it is damaged, do not treat that as the same warning; download the official package again and report the exact message and package name.

## Package verification

Release assets include `SHA256SUMS`. Compare the DMG's hash with the corresponding entry:

```sh
shasum -a 256 ~/Downloads/Clovercut-0.2.2-macos-arm64.dmg
```

Use your actual filename, including `x86_64` on Intel. Matching the checksum detects a damaged or mismatched download; it does not substitute for Apple notarization.

Future packages can be Developer ID signed and notarized once the maintainer supplies the required Apple credentials. The build pipeline already supports this, but an ad-hoc signature or successful CI launch alone does not establish Gatekeeper approval.
