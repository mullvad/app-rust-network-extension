#![allow(deprecated)]

use std::{fs::File, sync::OnceLock};

use block2::{DynBlock, RcBlock};
use env_logger::Target;
use hickory_resolver::{
    TokioResolver,
    config::{QUAD9, ResolverConfig},
    lookup::Lookup,
    net::{NetError, runtime::TokioRuntimeProvider},
    proto::{
        op::{Message, Query},
        serialize::binary::BinEncodable,
    },
};
use log::{error, info, trace};
use objc2::{ClassType, define_class, rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSError, NSString};
use objc2_network_extension::{
    NEAppProxyFlow, NEAppProxyUDPFlow, NEDNSProxyProvider, NEProvider, NEProviderStopReason,
    NWEndpoint, NWHostEndpoint,
};
use tokio::runtime::Runtime;

static RT: OnceLock<Runtime> = OnceLock::new();

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
            // Spawn the tokio runtime.
            RT.get_or_init(|| {
                Runtime::new().unwrap()
            });
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
            // TODO: Shutdown runtime?
            completion_handler.call(());
        }

        /// Override [`NEDNSProxyProvider::handleNewFlow`]
        ///
        /// A return value of `true` means we've decided to handle the flow.
        /// `false` means the flow should be terminated.
        #[unsafe(method(handleNewFlow:))]
        fn handle_new_flow(&self, flow: &NEAppProxyFlow) -> bool {
            // NOTE: Assume the flow is a UDP flow.
            // Cast it to NEAppProxyUDPFlow
            // TODO: Handle NEAppProxyTCPFlow
            let Some(udp_flow) = flow.downcast_ref::<NEAppProxyUDPFlow>() else {
                error!("flow was not a UDP flow - can't handle!");
                return false.into();
            };
            handle_new_udp_flow(udp_flow.into())
        }
    }
);

fn handle_new_udp_flow(flow: Retained<NEAppProxyUDPFlow>) -> bool {
    let local_endpoint = unsafe { flow.localEndpoint() };
    let local_endpoint = match local_endpoint {
        Some(endpoint) => endpoint.downcast::<NWHostEndpoint>().ok(),
        None => {
            error!("flow does not have a corresponding socket");
            None
        }
    };
    unsafe {
        flow.openWithLocalEndpoint_completionHandler(
            local_endpoint.as_deref(),
            &RcBlock::new(open_with_local_endpoint_completion_handler(flow.clone())),
        )
    };
    true
}

fn open_with_local_endpoint_completion_handler(
    flow: Retained<NEAppProxyUDPFlow>,
) -> impl Fn(*mut NSError) {
    move |err: *mut NSError| {
        if !err.is_null() {
            error!("Flow did not open succesfully");
            // TODO: Signal that we shouldn't handle flow.
            return;
        }
        unsafe {
            flow.readDatagramsWithCompletionHandler(&RcBlock::new(
                read_datagrams_completion_handler(flow.clone()),
            ))
        };
    }
}

fn read_datagrams_completion_handler(
    flow: Retained<NEAppProxyUDPFlow>,
) -> impl Fn(*mut NSArray<NSData>, *mut NSArray<NWEndpoint>, *mut NSError) {
    move |data: *mut NSArray<NSData>, endpoints: *mut NSArray<NWEndpoint>, err: *mut NSError| {
        if !err.is_null() {
            error!("Error reading data from UDP flow");
            return;
        }
        let datagrams = unsafe { &*data };
        let datagrams = datagrams.to_vec();
        let endpoints = unsafe { &*endpoints };
        let rt = RT.get().unwrap();
        rt.block_on(async {
            for (datagram, endpoint) in datagrams.iter().zip(endpoints) {
                let payload = datagram.to_vec();
                // Try to parse the UDP datagram into a DNS message.
                let dns = match Message::from_vec(&payload) {
                    Err(err) => {
                        error!("Failed to parse payload {payload:?}: {err}");
                        return;
                    }
                    Ok(dns) => dns,
                };
                let dns_resolver = Proxy::new();
                let mut messages = vec![];
                for query in &dns.queries {
                    let response = match dns_resolver.lookup(query).await {
                        Ok(response) => {
                            info!("Got response for query {query}");
                            response
                        }
                        Err(err) => {
                            error!("{err}");
                            continue;
                        }
                    };
                    // TODO: What happens if there are more than 1 query per datagram?
                    // Re-write the DNS response ID to match the original DNS request from
                    // the flow.
                    let message = {
                        let m = response.message();
                        let mut message = Message::new(
                            dns.metadata.id,
                            dns.metadata.message_type,
                            dns.metadata.op_code,
                        );
                        message.add_queries(m.queries.clone());
                        message.add_answers(m.answers.clone());
                        message.add_authorities(m.authorities.clone());
                        message.add_additionals(m.additionals.clone());
                        message.into_response()
                    };
                    messages.push(message);
                }
                // Write response back to flow
                trace!("Outbound DNS respone: {messages:?}");
                trace!("Writing back DNS response to flow");
                // TODO: Populate datagrams with DNS lookup response
                let messages = messages
                    .into_iter()
                    .map(|message| message.to_bytes().unwrap());
                let datagrams: Vec<_> = messages.map(NSData::from_vec).collect();
                let datagrams = NSArray::from_retained_slice(&datagrams);
                let endpoints = NSArray::from_retained_slice(&[endpoint]);
                unsafe {
                    flow.writeDatagrams_sentByEndpoints_completionHandler(
                        datagrams.as_ref(),
                        endpoints.as_ref(),
                        &RcBlock::new(write_datagrams_completion_handler(flow.clone())),
                    )
                }
            }
        });
    }
}

fn write_datagrams_completion_handler(flow: Retained<NEAppProxyUDPFlow>) -> impl Fn(*mut NSError) {
    move |err: *mut NSError| {
        if !err.is_null() {
            error!("Error writing datagrams back into flow");
        } else {
            info!("Successfully wrote DNS response back into flow")
        }
        let err = unsafe { &*err };
        unsafe { flow.closeReadWithError(Some(err)) };
        trace!("Closed flow for further reads");
        unsafe { flow.closeWriteWithError(Some(err)) };
        trace!("Closed flow for further writes");
    }
}

struct Proxy {
    resolver: TokioResolver,
}

impl Proxy {
    pub fn new() -> Proxy {
        let config = ResolverConfig::from_name_servers(QUAD9.udp().collect());
        let resolver = TokioResolver::builder_with_config(config, TokioRuntimeProvider::default())
            .build()
            .unwrap();
        Self { resolver }
    }

    pub async fn lookup(&self, query: &Query) -> Result<Lookup, NetError> {
        self.resolver
            .lookup(query.name().clone(), query.query_type())
            .await
    }
}

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
