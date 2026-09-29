use std::fs::File;

use block2::DynBlock;
use env_logger::Target;
use objc2::{ClassType, define_class, runtime::AnyObject};
use objc2_foundation::{NSDictionary, NSError, NSString};
use objc2_network_extension::{
    NEAppProxyFlow, NEDNSProxyProvider, NEProvider, NEProviderStopReason,
};

#[derive(Default)]
pub struct IVars {}

define_class!(
    #[unsafe(super(NEDNSProxyProvider))]
    #[ivars = IVars]
    #[name = "RustyDnsProxy"]
    pub struct RustyDnsProxy;

    impl RustyDnsProxy {
        /// Override [`NEDNSProxyProvider::startProxyWithOptions_completionHandler`]
        #[unsafe(method(startProxyWithOptions:completionHandler:))]
        fn start_proxy(
            &self,
            options: Option<&NSDictionary<NSString, AnyObject>>,
            completion_handler: &DynBlock<dyn Fn(*mut NSError)>,
        ) {
            log::info!("startProxy({options:?})");
            // null means success
            let error = std::ptr::null_mut();
            completion_handler.call((error,));
        }

        /// Override [`NEDNSProxyProvider::stopProxyWithReason_completionHandler`]
        #[unsafe(method(stopProxyWithReason:completionHandler:))]
        fn stop_proxy(
            &self,
            reason: NEProviderStopReason,
            completion_handler: &DynBlock<dyn Fn()>,
        ) {
            log::info!("stopProxy({reason:?})");
            completion_handler.call(());
        }

        /// Override [`NEDNSProxyProvider::handleNewFlow`]
        ///
        /// A return value of `true` means we've decided to handle the flow.
        /// `false` means the flow should be terminated.
        #[unsafe(method(handleNewFlow:))]
        fn handle_new_flow(&self, flow: &NEAppProxyFlow) -> bool {
            log::info!("handleNewFlow({flow:?})");
            false
        }
    }
);

fn main() {
    let log_file = File::create("/tmp/dnsproxy.log").expect("Failed to open log file");
    env_logger::builder()
        .format_timestamp_millis()
        .filter_level(log::LevelFilter::Trace)
        .target(Target::Pipe(Box::new(log_file)))
        .init();
    log::info!("Hello world!");

    log::info!("registering dns proxy class");
    // Force class registration so the Objective-C runtime knows about it.
    <RustyDnsProxy as ClassType>::class();

    log::info!("calling NEProvider::startSystemExtensionMode");
    unsafe { NEProvider::startSystemExtensionMode() };

    log::info!("driving dispatch queue");
    // Drive the main dispatch queue.
    dispatch2::dispatch_main();
}
