# This makefile assembles an app bundle containing a system extension bundle.
# 
# The filesystem layout requirements are strict, and looks like this:
# 
#   dnsproxy.app/
#   └── Contents/
#       ├── Info.plist
#       ├── MacOS/
#       │   └── dnsproxymanager
#       └── Library/
#           └── SystemExtensions/
#               └── dnsproxy.systemextension/
#                   └── Contents/
#                       ├── Info.plist
#                       └── MacOS/
#                           └── dnsproxy

CARGO_TARGET_DIR ?= target

# App bundle folder path. Must have an `.app` extension
APP := dnsproxy.app

# Extension bundle folder path. Must have a `.systemxtension` extension
EXT := $(APP)/Contents/Library/SystemExtensions/dnsproxy.systemextension

.PHONY: default clean

default: $(APP)/Contents/Info.plist \
         $(APP)/Contents/MacOS/dnsproxymanager \
         $(EXT)/Contents/Info.plist \
         $(EXT)/Contents/MacOS/dnsproxy
	# Signing must happen last, after both bundles are assembled.
	# System extension bundle must be signed before parent bundle.
	codesign --force --sign - --entitlements proxy.entitlements $(EXT)
	codesign --force --sign - --entitlements host.entitlements $(APP)

clean:
	rm -rf dnsproxy.app

$(APP)/Contents/MacOS $(EXT)/Contents/MacOS:
	mkdir -p $@

$(APP)/Contents/Info.plist: app.Info.plist | $(APP)/Contents/MacOS
	cp app.Info.plist $@

$(APP)/Contents/MacOS/dnsproxymanager: dnsproxymanager/src/main.rs dnsproxymanager/Cargo.toml | $(APP)/Contents/MacOS
	cargo build -p dnsproxymanager --release
	cp ${CARGO_TARGET_DIR}/release/dnsproxymanager $@

$(EXT)/Contents/Info.plist: extension.Info.plist | $(EXT)/Contents/MacOS
	cp extension.Info.plist $@

$(EXT)/Contents/MacOS/dnsproxy: src/main.rs Cargo.toml | $(EXT)/Contents/MacOS
	cargo build --release
	cp ${CARGO_TARGET_DIR}/release/dnsproxy $@
