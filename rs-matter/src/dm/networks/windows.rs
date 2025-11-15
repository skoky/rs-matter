use crate::dm::clusters::gen_diag::{InterfaceTypeEnum, NetifDiag, NetifInfo};
use crate::error::Error;
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use std::net::IpAddr;

pub struct WindowsNetnitfs {
}

impl WindowsNetnitfs {
    fn get() -> Vec<NetworkInterfaceWindows> {
        NetworkInterface::show().unwrap().iter().map(|i| {
            let mac = i.mac_addr.clone().unwrap();
            let mac_str = mac.split(":").collect::<Vec<&str>>();
            if mac_str.len() != 8 {
                panic!("MAC address must have 8 colon-separated parts (EUI-64 format).");
            }
            let mac = mac_str.iter().map(|e| u8::from_str_radix(e, 16).unwrap()).collect::<Vec<u8>>();
            let mac: [u8; 8] = mac.try_into().unwrap();


            let mut ip = vec![];
            let mut ipv6 = vec![];

            for iface in NetworkInterface::show()
                .expect("x1")
                .iter()
            {
                for addr in &iface.addr {
                    match addr.ip() {
                        IpAddr::V4(ipv4) => {
                            ip.push(ipv4);
                        }
                        IpAddr::V6(ipv6x) => {
                            ipv6.push(ipv6x);
                        }
                    }
                }
            }

            NetworkInterfaceWindows {
                name: i.name.clone(),
                operational: true,
                offprem_svc_reachable_ipv4: None,
                offprem_svc_reachable_ipv6: None,
                hw_addr: mac,
                ipv4_addr: ip,
                ipv6_addr: ipv6,
                index: i.index,
            }
        }).collect()
    }
}

impl NetifDiag for WindowsNetnitfs {
    fn netifs(&self, f: &mut dyn FnMut(&NetifInfo) -> Result<(), Error>) -> Result<(), Error> {
        for itf in WindowsNetnitfs::get() {
            f(&itf.to_net_info().unwrap())?;
        }

        Ok(())
    }
}

struct NetworkInterfaceWindows {
    name: String,
    operational: bool,
    offprem_svc_reachable_ipv4: Option<bool>,
    offprem_svc_reachable_ipv6: Option<bool>,
    hw_addr: [u8;8],
    ipv4_addr: Vec<core::net::Ipv4Addr>,
    ipv6_addr: Vec<core::net::Ipv6Addr>,
    index: u32,
}

impl NetworkInterfaceWindows {
    fn to_net_info(&self) -> Result<NetifInfo<'_>, Error> {

        Ok(NetifInfo {
            name: &self.name,
            operational: true,
            offprem_svc_reachable_ipv4: None,
            offprem_svc_reachable_ipv6: None,
            hw_addr: &self.hw_addr,
            ipv4_addrs: self.ipv4_addr.as_slice(),
            ipv6_addrs: self.ipv6_addr.as_slice(),
            netif_type: InterfaceTypeEnum::WiFi,
            netif_index: 0,
        })
    }
}

