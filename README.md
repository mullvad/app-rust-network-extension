# MacOS Network Extension written in Rust

This is an example of a DNS proxy provider network extension, written in rust, and managed by
another rust program.

## Development
You need `rust` and `make`. Running `make` will compile the rust programs, assemble them into a
systemextension bundle inside an app bundle, and self-sign them.

In order to run the network extension without involving Apple, you will need to break the shackles
on your system. Here's what worked for me, and I'll only recommend doing this in a VM:

**Disable System Integrity Protection**
- Boot into recovery mode.
	- `tart run --recovery <vm>`
- Go to Utilities -> Terminal
- Disable SIP, by running `csrutil disable`
- You may also need to run one of these commands:
  - Either
		- `nvram boot-args="amfi_get_out_of_my_way=0x1"`
  - ...or
		- `bputil --disable-boot-args-restriction`
		- `nvram 40A0DDD2-77F8-4392-B4A3-1E7304206516:boot-args='amfi=0x80'`
- `reboot`
- Run `csrutil status`, verify that SIP is disabled.

**Enable system extension developer mode**
- `systemextensionsctl developer on`

## Documentation
Some useful links to various pieces of documentation.

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
