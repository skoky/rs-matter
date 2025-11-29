use crate::error::{Error, ErrorCode};
use crate::transport::network::mdns::Service;
use crate::{Matter, MatterMdnsService};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::{HashMap, HashSet};
use std::{thread, time::Duration};

struct RegisteredMDnsDsService {}

pub struct MdnsDsResponder<'a> {
    matter: &'a Matter<'a>,
    services: HashMap<MatterMdnsService, MatterConfig>,
}

// --- Shared Configuration ---
// These values MUST match between QR Code and mDNS.
struct MatterConfig {
    discriminator: u16, // 12-bit (0-4095)
    passcode: u32,      // 27-bit
    vendor_id: u16,
    product_id: u16,
    instance_name: String,
    host_name: String,
    ip_address: String, // In production, detect this dynamically
    port: u16,
}

impl<'a> MdnsDsResponder<'a> {
    /// Create a new `MdnsDsResponder` for the given `Matter` instance.
    pub fn new(matter: &'a Matter<'a>) -> Self {
        Self {
            matter,
            services: HashMap::new(),
        }
    }

    /// Run the mDNS responder
    pub async fn run(&mut self) -> Result<(), Error> {
        loop {
            self.matter.wait_mdns().await;

            let mut services = HashSet::new();
            self.matter.mdns_services(|service| {
                services.insert(service);

                Ok(())
            })?;

            info!("mDNS services changed, updating...");

            self.update_services(&services).await?;

            info!("mDNS services updated");
        }
    }

    async fn update_services(
        &mut self,
        services: &HashSet<MatterMdnsService>,
    ) -> Result<(), Error> {
        for service in services {
            if !self.services.contains_key(service) {
                info!("Registering mDNS service: {:?}", service);
                let registered = self.register(service)?;
                self.services.insert(service.clone(), registered);
            }
        }

        // loop {
        //     let removed = self
        //         .services
        //         .iter()
        //         .find(|(service, _)| !services.contains(service));
        //
        //     if let Some((service, _)) = removed {
        //         info!("Deregistering mDNS service: {:?}", service);
        //         self.services.remove(&service.clone());
        //     } else {
        //         break;
        //     }
        // }

        Ok(())
    }

    fn register(&mut self, service: &MatterMdnsService) -> Result<MatterConfig, Error> {
        let service = Service::call_with(
            service,
            self.matter.dev_det(),
            self.matter.port(),
            |service| {
                let main_service_type = format!("{}.{}.local.", service.service, service.protocol);
                let mdns = ServiceDaemon::new().expect("Failed to create mDNS daemon");
                let mut props = HashMap::new();

                for kvs in service.txt_kvs {
                    // println!("mDNS TXT key {} val {}", kvs.0, kvs.1);
                    props.insert(kvs.0.to_string(), kvs.1.to_string());
                }

                let mst = "_L3840,_S15,_V65521P32769,_CM._matterc._udp.local.";

                let main_info = ServiceInfo::new(
                    main_service_type.as_str(),
                    "432AB907F2F5D3EF",
                    "nuc.local.",         // config.host_name.as_str(),
                    "10.0.10.117", // config.ip_address.clone(),
                    service.port,  // config.port,
                    props.clone(),           // &txt_refs[..],
                )
                .expect("Valid main service");
                mdns.register(main_info).expect("Failed to register Main");

                let info = ServiceInfo::new(
                    mst,
                    "432AB907F2F5D3EF",
                    "nuc.local.",
                    "10.0.10.117",
                    service.port,
                    props.clone(),
                ).unwrap();
                mdns.register(info).expect("Failed to register subtype");

                for kvs in service.txt_kvs {
                    let sub_type = format!("_{}{}._sub.{}", kvs.0.to_string(), kvs.1.to_string(), main_service_type);
                    println!("Subtype: {sub_type}");
                    let info = ServiceInfo::new(
                        &sub_type,
                        "432AB907F2F5D3EF",
                        "nuc.local.",
                        "10.0.10.117",
                        service.port,
                        props.clone(),
                    )
                        .unwrap();
                    mdns.register(info).expect("Failed to register subtype");
                }

                Ok(())
            },
        );

        // [2025-11-29T18:12:11Z INFO  rs_matter::transport::network::mdns::mdns_windows] Registering mDNS service: Commissionable { id: 7099960846431634799, discriminator: 3840 }
        // Subtype: _D3840._sub._matterc._udp.local.
        //     Subtype: _CM1._sub._matterc._udp.local.
        //     Subtype: _VP65521+32769._sub._matterc._udp.local.
        //     Subtype: _SAI300._sub._matterc._udp.local.
        //     Subtype: _SII5000._sub._matterc._udp.local.
        //     Subtype: _DNMyTest._sub._matterc._udp.local.
        //     registering end

        let config = MatterConfig {
            discriminator: 0,
            passcode: 0,
            vendor_id: 0,
            product_id: 0,
            instance_name: "".to_string(),
            host_name: "".to_string(),
            ip_address: "".to_string(),
            port: 0,
        };
        println!("registering end");

        Ok(config)
    }
}

fn start_mdns_service(config: &MatterConfig) {
    let mdns = ServiceDaemon::new().expect("Failed to create mDNS daemon");
    let main_service_type = "_matterc._udp.local.";

    // Common TXT Records
    let txt_properties = [
        ("D", config.discriminator.to_string()),
        ("CM", "1".to_string()), // 1 = Normal Commissioning
        ("VP", format!("{}+{}", config.vendor_id, config.product_id)),
        ("DN", "RustMatterNode".to_string()),
        ("DT", "1".to_string()), // Device Type
        ("SII", "5000".to_string()),
        ("SAI", "300".to_string()),
    ];

    // Need to convert to strict signature for mdns-sd
    let txt_refs: Vec<(&str, &str)> = txt_properties
        .iter()
        .map(|(k, v)| (*k, v.as_str()))
        .collect();

    // 1. Register Main Service
    let main_info = ServiceInfo::new(
        main_service_type,
        config.instance_name.as_str(),
        config.host_name.as_str(),
        config.ip_address.clone(),
        config.port,
        &txt_refs[..],
    )
    .expect("Valid main service");
    mdns.register(main_info).expect("Failed to register Main");

    // 2. Register Subtypes
    // Helper to reduce repetition
    let reg_sub = |prefix: String| {
        let sub_type = format!("{}{}", prefix, main_service_type);
        let info = ServiceInfo::new(
            &sub_type,
            config.instance_name.as_str(),
            config.host_name.as_str(),
            config.ip_address.clone(),
            config.port,
            &txt_refs[..],
        )
        .unwrap();
        mdns.register(info).expect("Failed to register subtype");
        println!("  -> Registered: {}", sub_type);
    };

    // A. Long Discriminator: _L<12-bit>
    reg_sub(format!("_L{}._sub.", config.discriminator));

    // B. Short Discriminator: _S<Upper-4-bits>
    let short_d = (config.discriminator >> 8) & 0x0F;
    reg_sub(format!("_S{}._sub.", short_d));

    // C. Commissioning Mode: _CM
    reg_sub("_CM._sub.".to_string());

    // D. Vendor ID: _V<VID>
    reg_sub(format!("_V{}._sub.", config.vendor_id));

    println!("mDNS Active. Waiting for controller...");

    // Keep thread alive
    loop {
        thread::sleep(Duration::from_secs(10));
    }
}
