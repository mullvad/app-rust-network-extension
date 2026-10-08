# Rust Network Extension

This is an example of a macOS DNS proxy provider network extension, written in Rust, and managed by
another Rust program.

## Development
You need `rust` and `nu`. Running `./build.nu` will compile the rust programs, assemble them into a
systemextension bundle inside an app bundle, and sign them.

### Signing

Apple places certain shackles on macOS systems: _AMFI_ (Apple Mobile File Integrity) and
_SIP_ (System Integrity Protection). You may wish to disable these to simplify development
and/or remove the requirement to have a paid Apple developer account.

If you want to build and run this demo without signing with an Apple-approved certificate,
you must disable AMFI.

If you want to build and run this demo without uploading it to Apple for notarization,
you must also disable SIP and enable system extension developer mode.

**Disable AMFI**

If you are self-signing the bundle (running the build script without `--pem`), then you will need
to disable AMFI on your system, as AMFI blocks the system from running unsigned code.

First, reboot into recovery mode.
Then, run one of these commands:

    nvram boot-args="amfi_get_out_of_my_way=0x1"

...or

    bputil --disable-boot-args-restriction`
    nvram 40A0DDD2-77F8-4392-B4A3-1E7304206516:boot-args='amfi=0x80'`

This worked for me, but your mileage may vary.
If macOS SIGKILLs your applications with a signature error, this command may tell you the reason:

    log stream --predicate 'process == "amfid" OR process == "taskgated-helper"'

**Disable SIP**

- Boot into recovery mode: `tart run --recovery <vm>`
- Go to Utilities -> Terminal
- Disable SIP: `csrutil disable`
- `reboot`
- Verify that SIP is disabled: `csrutil status`

**Enable system extension developer mode**

Run `systemextensionsctl developer on` in a terminal.

### Setting up signing keys

If you don't feel like disabling AMFI, follow these steps.

```sh
# Generate a private key.
openssl genrsa -out me@example.com.pem 2048

# Generate certificate signing request. Upload this to developers.apple.com.
rcodesign generate-certificate-signing-request --pem-file me@example.com.pem --csr-pem-file me@example.com.csr
```

Apple will give you a `cer`-file in return. Let's call it `me@example.com.cer`.
You will also need to create and download provisioning profile. The profile is tied to
both your developer account, and your macOS Provisioning UDID. Additionally, the profile must grant
the "Network Extension" and "System Extension" entitlements, as these are required for this example.
Let's call the profile `dnsproxy.provisionprofile`.

The provisioning profile must be valid for...
1. ...the developer who is signing the bundle.
2. ...and the devices you intend on running the program on.

Note that this example requires the System Extension and the parent app to have the same Bundle ID.
Thus, the provisioning profile must be valid for both. This is nonstandard.

Finally, run the following command to build and sign this example.

```sh
# Build...
./build.nu --pem ./me@example.com.pem --cer ./me@example.com.cer --provision-profile ./dnsproxy.provisionprofile

# ..and run
./net.mullvad.MullvadVPN.DNSProxy.app/Contents/MacOS/dnsproxymanager
```

### Notarization

If you don't want to disable SIP, the bundle must be notarized by Apple.

_(TODO: describe how to do this)_

## Documentation
Some useful links to various pieces of documentation.

### `apple-codesign` and `rcodesign`
- [Apple Code Signing](https://gregoryszorc.com/docs/apple-codesign/main/index.html)
- [Managing Code Signing Certificates](https://gregoryszorc.com/docs/apple-codesign/main/apple_codesign_certificate_management.html)

### Apple - Network Extension API
- [`NEDNSProxyProvider`](https://developer.apple.com/documentation/networkextension/nednsproxyprovider)
- [`NEDNSProxyManager`](https://developer.apple.com/documentation/networkextension/nednsproxymanager)
- [`NEDNSProxyProviderProtocol`](https://developer.apple.com/documentation/networkextension/nednsproxyproviderprotocol)
- [`NEVPNProtocol`](https://developer.apple.com/documentation/networkextension/nevpnprotocol)
- [`NEProvider.startSystemExtensionMode()`](https://developer.apple.com/documentation/networkextension/neprovider/startsystemextensionmode())

### Apple - System Extensions
- [System Extensions](https://developer.apple.com/documentation/systemextensions)
- [Installing System Extensions and Drivers](https://developer.apple.com/documentation/systemextensions/installing-system-extensions-and-drivers)

### Apple - App-extension-style & Info.plist keys
- [`NSExtension`](https://developer.apple.com/documentation/bundleresources/information_property_list/nsextension)
- [`NSExtensionPointIdentifier`](https://developer.apple.com/documentation/bundleresources/information_property_list/nsextension/nsextensionpointidentifier)
- [`NSExtensionPrincipalClass`](https://developer.apple.com/documentation/bundleresources/information_property_list/nsextension/nsextensionprincipalclass)

### Apple - Bundles
- [About Bundles](https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/AboutBundles/AboutBundles.html)
- [`NSBundle.mainBundle`](https://developer.apple.com/documentation/foundation/bundle/main)

### Rust crates
- [`objc2`](https://docs.rs/objc2)
- [`objc2-network-extension`](https://docs.rs/objc2-network-extension)
- [`objc2-system-extensions`](https://docs.rs/objc2-system-extensions)
