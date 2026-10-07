#!/usr/bin/env nu

const APP_PLIST = "./app.Info.plist"
const EXT_PLIST = "./ext.Info.plist"

# Build, bundle, and sign the DNS proxy example.
def main [
  --pem: path # Path to the developer private signing key (.pem)
  --cer: path # Path to the Apple-signed certificate for the developer (.cer)
  --provision-profile: path # Path to the Apple-provided provisioning profile
]: nothing -> nothing {
  let do_sign = [$pem $cer] | all ($it != null)
  let dont_sign = [$pem $cer] | all ($it == null)

  if not ($do_sign or $dont_sign) {
    return (error make -u "Must specify both or none: [--pem --cer]")
  }

  if $dont_sign and $provision_profile != null {
    return (error make -u "--provision-profile requires [--pem --cer]")
  }

  # Extract Bundle IDs from Info.plist-files
  let appid = (plutil -extract "CFBundleIdentifier" raw -o - $APP_PLIST)
  let extid = (plutil -extract "CFBundleIdentifier" raw -o - $EXT_PLIST)

  let app = $"./($appid).app"
  let ext = $"($app)/Contents/Library/SystemExtensions/($extid).systemextension"

  # Start with a clean slate
  rm -rf $app

  # Build directory structure
  print $"Creating bundle directory structure"
  mkdir $"($app)/Contents/MacOS"
  mkdir $"($ext)/Contents/MacOS"

  # Copy plists into place
  print $"Copying plists"
	cp app.Info.plist $"($app)/Contents/Info.plist"
	cp ext.Info.plist $"($ext)/Contents/Info.plist"

  # Compile rust binaries and copy them into place.
  print $"Building rust binaries"
  let target = ($env | get -o CARGO_TARGET_DIR | default "target")
	cargo build -p dnsproxymanager --release
	cargo build --release
  cp $"($target)/release/dnsproxymanager" $"($app)/Contents/MacOS/"
  cp $"($target)/release/dnsproxy"        $"($ext)/Contents/MacOS/"

  if $do_sign {
    # Place provisioning profile
    if $provision_profile != null {
      # NOTE: this presumes that both bundles have the same ID
      # TODO: this presumption is probably bad
      if $appid != $extid {
        return (error make -u $"Bundle IDs must match: ($appid) != ($extid)")
      }
      
      print $"Using provisioning profile ($provision_profile)"
      cp $provision_profile $"($app)/Contents/embedded.provisionprofile"
      cp $provision_profile $"($ext)/Contents/embedded.provisionprofile"
    }

  	# Sign bundles recursively using provided key.
    print $"Signing with ($pem) and ($cer)"
    # TODO: --entitlements-xml-file
    rcodesign sign --shallow --certificate-der-file $cer --pem-file $pem --entitlements-xml-file ./ext.entitlements $ext
    rcodesign sign --shallow --certificate-der-file $cer --pem-file $pem --entitlements-xml-file ./app.entitlements $app
  } else {
  	# Self-sign bundles. Must happen last, after both bundles are assembled.
  	# System extension bundle must be signed before parent bundle.
    print $"No developer keys provided. Self-signing instead."
  	codesign --force --sign - --entitlements app.entitlements $app
  	codesign --force --sign - --entitlements ext.entitlements $ext
  }
}
