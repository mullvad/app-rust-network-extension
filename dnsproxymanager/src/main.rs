use std::{
    ptr::NonNull,
    sync::{LazyLock, mpsc},
    time::Duration,
};

use block2::RcBlock;
use clap::Parser;
use dispatch2::DispatchQueue;
use objc2::{
    AnyThread as _, DefinedClass, define_class, msg_send, rc::Retained, runtime::ProtocolObject,
};
use objc2_foundation::{NSArray, NSError, NSObject, NSObjectProtocol, NSString, ns_string};
use objc2_network_extension::{NEDNSProxyManager, NEDNSProxyProviderProtocol};
use objc2_system_extensions::{
    OSSystemExtensionManager, OSSystemExtensionProperties, OSSystemExtensionReplacementAction,
    OSSystemExtensionRequest, OSSystemExtensionRequestDelegate, OSSystemExtensionRequestResult,
};

// TODO:
// This file contains an abundance of `unsafe` operations, which need SAFETY docs.
// Most of these are calls into the objective-c runtime.

#[derive(Parser)]
struct Opt {
    #[clap(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Install and enable the network extension.
    Enable,
    /// Disable and remove the network extension. Must be executed from a graphical session.
    Disable,
}

// NOTE: Keep in sync with ../../ext.Info.plist
const EXTENSION_BUNDLE_ID: LazyLock<&NSString> =
    LazyLock::new(|| ns_string!("net.mullvad.MullvadVPN.DNSProxy"));

fn main() {
    let opt = Opt::parse();

    env_logger::builder()
        .format_timestamp_millis()
        .filter_level(log::LevelFilter::Trace)
        .init();

    std::thread::spawn(move || {
        match opt.command {
            Command::Enable => install_and_enable(),
            Command::Disable => remove_and_disable(),
        }
        std::process::exit(0);
    });

    // Run the dispatch queue on the main thread.
    // TODO: This is not very cash-money. Can we move this to a different thread?
    dispatch2::dispatch_main();
}

/// Helper for wrapping a completion (callback) closure in an RcBlock,
/// and sending the result on an mpsc-channel.
macro_rules! callback {
    (move |$args:tt| $block:block) => {{
        let (tx, rx) = mpsc::channel();
        let block = RcBlock::new(move |$args| {
            _ = tx.send($block);
        });
        (block, rx)
    }};
}

fn remove_and_disable() {
    log::info!("Removing DNS proxy provider system extension");

    let extension_manager = unsafe { OSSystemExtensionManager::sharedManager() };

    // Send an OSSystemExtensionRequest to activate the dnsproxy extension.
    // This should result in `Message::ExtensionLoaded`
    let queue = DispatchQueue::main();
    let request = unsafe {
        OSSystemExtensionRequest::deactivationRequestForExtension_queue(*EXTENSION_BUNDLE_ID, queue)
    };
    let (tx, rx) = mpsc::channel();
    let delegate = SERequestDelegate::new(SERequestDelegateVars { tx: tx.clone() });
    let delegate = ProtocolObject::from_retained(delegate);
    unsafe { request.setDelegate(Some(&delegate)) };
    unsafe { extension_manager.submitRequest(&request) };

    // Wait until request completes.
    loop {
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(SEMessage::Finished(result)) => {
                log::info!("System extension removal finished ({result:?})");
                break;
            }
            Ok(SEMessage::Progress(event)) => {
                log::info!("Progress: {event}");
            }
            Ok(SEMessage::Failed(error)) => {
                log::error!("Deactivation failed: {error}");
                log::info!(
                    "NOTE: if `error 13`, ensure you are executing this command from within a graphical user session."
                );
                std::process::exit(1);
            }
            Err(_) => {
                log::error!("Timed out waiting for system extension result");
                std::process::exit(124);
            }
        }
    }
}

fn install_and_enable() {
    log::info!("Installing DNS proxy provider system extension");

    let extension_manager = unsafe { OSSystemExtensionManager::sharedManager() };

    // Send an OSSystemExtensionRequest to activate the dnsproxy extension.
    // This should result in `Message::ExtensionLoaded`
    let queue = DispatchQueue::main();
    let request = unsafe {
        OSSystemExtensionRequest::activationRequestForExtension_queue(*EXTENSION_BUNDLE_ID, queue)
    };
    let (tx, rx) = mpsc::channel();
    let delegate = SERequestDelegate::new(SERequestDelegateVars { tx: tx.clone() });
    let delegate = ProtocolObject::from_retained(delegate);
    unsafe { request.setDelegate(Some(&delegate)) };
    unsafe { extension_manager.submitRequest(&request) };

    // Wait until request completes.
    loop {
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(SEMessage::Finished(result)) => {
                log::info!("System extension activation finished ({result:?})");
                break;
            }
            Ok(SEMessage::Progress(event)) => {
                log::info!("Progress: {event}");
            }
            Ok(SEMessage::Failed(error)) => {
                log::error!("Activation failed: {error}");
                std::process::exit(1);
            }
            Err(_) => {
                log::error!("Timed out waiting for system extension result");
                std::process::exit(124);
            }
        }
    }

    log::info!("Loading DNS preferences");
    let dns_manager = unsafe { NEDNSProxyManager::sharedManager() };
    let (completion, rx) = callback!(move |error| {
        match NonNull::new(error) {
            None => Ok(()),
            Some(error) => Err(format!("{:?}", unsafe { error.as_ref() })),
        }
    });
    unsafe { dns_manager.loadFromPreferencesWithCompletionHandler(&completion) };

    match rx.recv_timeout(Duration::from_secs(30)) {
        // Our `NEDNSProxyManager` has loaded preferences successfully.
        Ok(Ok(())) => {}
        Ok(Err(message)) => {
            log::error!("Failed to load DNS preferences: {message}");
            std::process::exit(1);
        }
        Err(_timeout) => {
            log::error!("Timeout");
            std::process::exit(1);
        }
    }

    log::info!("DNS preferences loaded");
    let (completion, rx) = callback!(move |error| {
        match NonNull::new(error) {
            None => Ok(()),
            Some(error) => Err(format!("{:?}", unsafe { error.as_ref() })),
        }
    });

    let was_enabled = unsafe { dns_manager.isEnabled() };
    log::info!("DNS proxy provider was enabled: {was_enabled}");

    // Configure and enable DNS proxy provider
    log::info!("Enabling DNS proxy provider and saving preferences");
    let config = unsafe { NEDNSProxyProviderProtocol::new() };
    unsafe { config.setProviderConfiguration(None) }; // configuration that can be passed to the system extension, i think...
    unsafe { config.setProviderBundleIdentifier(Some(*EXTENSION_BUNDLE_ID)) };
    unsafe { config.setServerAddress(Some(ns_string!("1.1.1.1"))) };
    unsafe { dns_manager.setEnabled(true) };
    unsafe { dns_manager.setProviderProtocol(Some(&config)) };
    unsafe { dns_manager.saveToPreferencesWithCompletionHandler(&completion) };

    log::warn!("This may require user confirmation!");
    match rx.recv_timeout(Duration::from_secs(30)) {
        // Our `NEDNSProxyManager` has saved preferences successfully.
        Ok(Ok(())) => {}
        Ok(Err(message)) => {
            log::error!("Failed to save DNS preferences: {message}");
            std::process::exit(1);
        }
        Err(_timeout) => {
            log::error!("Timeout");
            std::process::exit(1);
        }
    }

    log::info!("DNS preferences saved successfully");
}

#[derive(Debug)]
pub enum SEMessage {
    Finished(OSSystemExtensionRequestResult),
    // Progress update for an ongoing operation that has not completed yet.
    Progress(String),
    Failed(String),
}

pub struct SERequestDelegateVars {
    tx: mpsc::Sender<SEMessage>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = SERequestDelegateVars]
    pub struct SERequestDelegate;
    unsafe impl NSObjectProtocol for SERequestDelegate {}

    unsafe impl OSSystemExtensionRequestDelegate for SERequestDelegate {
        #[unsafe(method(request:actionForReplacingExtension:withExtension:))]
        fn request_action_for_replacing_extension_with_extension(
            &self,
            request: &OSSystemExtensionRequest,
            existing: &OSSystemExtensionProperties,
            ext: &OSSystemExtensionProperties,
        ) -> OSSystemExtensionReplacementAction {
            log::debug!(
                "request:actionForReplacingExtension:withExtension:({existing:?}, {ext:?})"
            );
            self.tx(SEMessage::Progress(format!("actionForReplacingExtension")));
            OSSystemExtensionReplacementAction::Replace
        }

        #[unsafe(method(requestNeedsUserApproval:))]
        fn request_needs_user_approval(&self, request: &OSSystemExtensionRequest) {
            let identifier = unsafe { request.identifier() };
            self.tx(SEMessage::Progress(format!(
                "Needs user approval for {identifier}"
            )));
        }

        #[unsafe(method(request:didFinishWithResult:))]
        fn request_did_finish_with_result(
            &self,
            request: &OSSystemExtensionRequest,
            result: OSSystemExtensionRequestResult,
        ) {
            let identifier = unsafe { request.identifier() };
            log::debug!("request:didFinishWithResult:({identifier:?}, {result:?})");
            self.tx(SEMessage::Finished(result));
        }

        #[unsafe(method(request:didFailWithError:))]
        fn request_did_fail_with_error(&self, request: &OSSystemExtensionRequest, error: &NSError) {
            let identifier = unsafe { request.identifier() };
            log::debug!("request:didFailWithError:({identifier:?}, {error:?})",);
            self.tx(SEMessage::Failed(format!("{error:?}")));
        }

        #[unsafe(method(request:foundProperties:))]
        fn request_found_properties(
            &self,
            request: &OSSystemExtensionRequest,
            properties: &NSArray<OSSystemExtensionProperties>,
        ) {
            let identifier = unsafe { request.identifier() };
            log::debug!("request:foundProperties:({identifier:?}, {properties:?})",);
            self.tx(SEMessage::Progress(format!("Found properties")));
        }
    }
);

// Add creation method.
impl SERequestDelegate {
    fn new(vars: SERequestDelegateVars) -> Retained<Self> {
        // Initialize instance variables.
        let this = Self::alloc().set_ivars(vars);
        // Call `NSObject`'s `init` method.
        unsafe { msg_send![super(this), init] }
    }

    /// Send `message` on the mpsc channel.
    fn tx(&self, message: SEMessage) {
        _ = self.ivars().tx.send(message);
    }
}
