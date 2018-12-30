#[cfg(feature = "diesel")]
#[macro_use]
extern crate diesel;

use std::cmp;
use std::error::Error;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[cfg(feature = "diesel")]
/// Support for Diesel PostgreSQL CIDR type
pub mod diesel_support;
mod helpers;
/// `Ipv4RangeIterator`, `Ipv4NetworkIterator` and `Ipv6NetworkIterator`
pub mod iterator;
#[cfg(any(feature = "diesel", feature = "postgres"))]
mod postgres_common;
#[cfg(feature = "postgres")]
mod postgres_support;

/// IPv6 Multicast Address Scopes
#[derive(Copy, PartialEq, Eq, Clone, Hash, Debug)]
pub enum Ipv6MulticastScope {
    InterfaceLocal,
    LinkLocal,
    RealmLocal,
    AdminLocal,
    SiteLocal,
    OrganizationLocal,
    Global,
}

/// Holds IPv4 or IPv6 network
#[derive(Clone, Eq, PartialEq, Debug, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum IpNetwork {
    V4(Ipv4Network),
    V6(Ipv6Network),
}

impl IpNetwork {
    /// Constructs new `IpNetwork` based on [`IpAddr`] and `netmask`.
    ///
    /// [`IpAddr`]: https://doc.rust-lang.org/std/net/enum.IpAddr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr};
    /// use std::str::FromStr;
    /// use ip_network::{IpNetwork, Ipv4Network};
    ///
    /// let network_address = IpAddr::from_str("192.168.1.0").unwrap();
    /// let ip_network = IpNetwork::new(network_address, 24).unwrap();
    /// assert_eq!(ip_network, IpNetwork::V4(Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap()));
    /// ```
    pub fn new<I: Into<IpAddr>>(network_address: I, netmask: u8) -> Result<Self, IpNetworkError> {
        Ok(match network_address.into() {
            IpAddr::V4(ip) => IpNetwork::V4(Ipv4Network::new(ip, netmask)?),
            IpAddr::V6(ip) => IpNetwork::V6(Ipv6Network::new(ip, netmask)?),
        })
    }

    /// Constructs new `IpNetwork` based on [`IpAddr`] and `netmask` with truncating host bits
    /// from given `network_address`.
    ///
    /// Returns error if netmask is bigger than 32 for IPv4 and 128 for IPv6.
    ///
    /// [`Ipv4Addr`]: https://doc.rust-lang.org/std/net/struct.IpAddr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr};
    /// use ip_network::IpNetwork;
    ///
    /// let network_address = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 128));
    /// let ip_network = IpNetwork::new_truncate(network_address, 24).unwrap();
    /// assert_eq!(ip_network.network_address(), IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)));
    /// assert_eq!(ip_network.netmask(), 24);
    /// ```
    pub fn new_truncate<I: Into<IpAddr>>(network_address: I, netmask: u8) -> Result<Self, IpNetworkError> {
        Ok(match network_address.into() {
            IpAddr::V4(ip) => IpNetwork::V4(Ipv4Network::new_truncate(ip, netmask)?),
            IpAddr::V6(ip) => IpNetwork::V6(Ipv6Network::new_truncate(ip, netmask)?),
        })
    }

    /// Returns network IP address.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr};
    /// use ip_network::IpNetwork;
    ///
    /// let ip_network = IpNetwork::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert_eq!(ip_network.network_address(), IpAddr::V4(Ipv4Addr::new(192, 168, 1, 0)));
    /// ```
    pub fn network_address(&self) -> IpAddr {
        match *self {
            IpNetwork::V4(ref ip_network) => IpAddr::V4(ip_network.network_address),
            IpNetwork::V6(ref ip_network) => IpAddr::V6(ip_network.network_address),
        }
    }

    /// Returns network mask as integer.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr};
    /// use ip_network::IpNetwork;
    ///
    /// let ip_network = IpNetwork::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert_eq!(ip_network.netmask(), 24);
    /// ```
    pub fn netmask(&self) -> u8 {
        match *self {
            IpNetwork::V4(ref ip_network) => ip_network.netmask,
            IpNetwork::V6(ref ip_network) => ip_network.netmask,
        }
    }

    /// Returns `true` if `IpNetwork` contains `Ipv4Network` struct.
    pub fn is_ipv4(&self) -> bool {
        match *self {
            IpNetwork::V4(_) => true,
            IpNetwork::V6(_) => false,
        }
    }

    /// Returns `true` if `IpNetwork` contains `Ipv6Network` struct.
    pub fn is_ipv6(&self) -> bool {
        !self.is_ipv4()
    }

    /// Returns `true` if `IpNetwork` contains `IpAddr`. For different network type
    /// (for example IpNetwork is IPv6 and IpAddr is IPv4) always returns `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    /// use ip_network::IpNetwork;
    ///
    /// let ip_network = IpNetwork::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert!(ip_network.contains(Ipv4Addr::new(192, 168, 1, 25)));
    /// assert!(!ip_network.contains(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 1, 0, 0)));
    /// ```
    pub fn contains<I: Into<IpAddr>>(&self, ip: I) -> bool {
        match (self, ip.into()) {
            (IpNetwork::V4(network), IpAddr::V4(ip)) => network.contains(ip),
            (IpNetwork::V6(network), IpAddr::V6(ip)) => network.contains(ip),
            _ => false,
        }
    }

    /// Returns `true` if the network is part of multicast network range.
    pub fn is_multicast(&self) -> bool {
        match *self {
            IpNetwork::V4(ref ip_network) => ip_network.is_multicast(),
            IpNetwork::V6(ref ip_network) => ip_network.is_multicast(),
        }
    }

    /// Returns `true` if this is a part of network reserved for documentation.
    pub fn is_documentation(&self) -> bool {
        match *self {
            IpNetwork::V4(ref ip_network) => ip_network.is_documentation(),
            IpNetwork::V6(ref ip_network) => ip_network.is_documentation(),
        }
    }

    /// Returns `true` if this network is inside loopback address range.
    pub fn is_loopback(&self) -> bool {
        match *self {
            IpNetwork::V4(ref ip_network) => ip_network.is_loopback(),
            IpNetwork::V6(ref ip_network) => ip_network.is_loopback(),
        }
    }

    /// Returns `true` if the network appears to be globally routable.
    pub fn is_global(&self) -> bool {
        match *self {
            IpNetwork::V4(ref ip_network) => ip_network.is_global(),
            IpNetwork::V6(ref ip_network) => ip_network.is_global(),
        }
    }
}

impl fmt::Display for IpNetwork {
    /// Converts `IpNetwork` to string in format X.X.X.X/Y for IPv4 and X:X::X/Y for IPv6 (CIDR notation).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::{IpNetwork, Ipv4Network};
    ///
    /// let ip_network = IpNetwork::V4(Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap());
    /// assert_eq!(ip_network.to_string(), "192.168.1.0/24");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IpNetwork::V4(ref network) => network.fmt(f),
            IpNetwork::V6(ref network) => network.fmt(f),
        }
    }
}

impl FromStr for IpNetwork {
    type Err = IpNetworkParseError;

    /// Converts string in format IPv4 (X.X.X.X/Y) or IPv6 (X:X::X/Y) CIDR notation to `IpNetwork`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use std::str::FromStr;
    /// use ip_network::{IpNetwork, Ipv4Network};
    ///
    /// let ip_network = IpNetwork::from_str("192.168.1.0/24").unwrap();
    /// assert_eq!(ip_network, IpNetwork::V4(Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap()));
    /// ```
    fn from_str(s: &str) -> Result<IpNetwork, IpNetworkParseError> {
        let (ip, netmask) =
            helpers::split_ip_netmask(s).ok_or(IpNetworkParseError::InvalidFormatError)?;

        let netmask =
            u8::from_str(netmask).map_err(|_| IpNetworkParseError::InvalidNetmaskFormat)?;

        if let Ok(network_address) = Ipv4Addr::from_str(ip) {
            let network = Ipv4Network::new(network_address, netmask)
                .map_err(IpNetworkParseError::IpNetworkError)?;
            Ok(IpNetwork::V4(network))
        } else if let Ok(network_address) = Ipv6Addr::from_str(ip) {
            let network = Ipv6Network::new(network_address, netmask)
                .map_err(IpNetworkParseError::IpNetworkError)?;
            Ok(IpNetwork::V6(network))
        } else {
            Err(IpNetworkParseError::AddrParseError)
        }
    }
}

impl From<Ipv4Addr> for IpNetwork {
    /// Converts `Ipv4Addr` to `IpNetwork` with netmask 32.
    fn from(ip: Ipv4Addr) -> Self {
        IpNetwork::V4(Ipv4Network::from(ip))
    }
}

impl From<Ipv6Addr> for IpNetwork {
    /// Converts `Ipv46ddr` to `IpNetwork` with netmask 128.
    fn from(ip: Ipv6Addr) -> Self {
        IpNetwork::V6(Ipv6Network::from(ip))
    }
}

impl From<IpAddr> for IpNetwork {
    /// Converts `IpAddr` to `IpNetwork` with netmask 32 for IPv4 address and 128 for IPv6 address.
    fn from(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V4(ip) => IpNetwork::from(ip),
            IpAddr::V6(ip) => IpNetwork::from(ip),
        }
    }
}

impl From<Ipv4Network> for IpNetwork {
    fn from(network: Ipv4Network) -> Self {
        IpNetwork::V4(network)
    }
}

impl From<Ipv6Network> for IpNetwork {
    fn from(network: Ipv6Network) -> Self {
        IpNetwork::V6(network)
    }
}

/// IPv4 Network
#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Ipv4Network {
    network_address: Ipv4Addr,
    netmask: u8,
}

impl Ipv4Network {
    /// IPv4 address length in bits.
    const LENGTH: u8 = 32;

    /// Constructs new `Ipv4Network` based on [`Ipv4Addr`] and `netmask`.
    ///
    /// Returns error if netmask is bigger than 32 or if host bits are set in `network_address`.
    ///
    /// [`Ipv4Addr`]: https://doc.rust-lang.org/std/net/struct.Ipv4Addr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert_eq!(ip_network.network_address(), Ipv4Addr::new(192, 168, 1, 0));
    /// assert_eq!(ip_network.netmask(), 24);
    /// ```
    pub fn new(network_address: Ipv4Addr, netmask: u8) -> Result<Self, IpNetworkError> {
        if netmask > Self::LENGTH {
            return Err(IpNetworkError::NetmaskError(netmask));
        }

        if u32::from(network_address).trailing_zeros() < (Self::LENGTH as u32 - netmask as u32) {
            return Err(IpNetworkError::HostBitsSet);
        }

        Ok(Self {
            network_address,
            netmask,
        })
    }

    /// Constructs new `Ipv4Network` based on [`Ipv4Addr`] and `netmask` with truncating host bits
    /// from given `network_address`.
    ///
    /// Returns error if netmask is bigger than 32.
    ///
    /// [`Ipv4Addr`]: https://doc.rust-lang.org/std/net/struct.Ipv4Addr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new_truncate(Ipv4Addr::new(192, 168, 1, 100), 24).unwrap();
    /// assert_eq!(ip_network.network_address(), Ipv4Addr::new(192, 168, 1, 0));
    /// assert_eq!(ip_network.netmask(), 24);
    /// ```
    pub fn new_truncate(network_address: Ipv4Addr, netmask: u8) -> Result<Self, IpNetworkError> {
        if netmask > Self::LENGTH {
            return Err(IpNetworkError::NetmaskError(netmask));
        }

        let network_address =
            Ipv4Addr::from(u32::from(network_address) & helpers::get_bite_mask(netmask));

        Ok(Self {
            network_address,
            netmask,
        })
    }

    /// Returns network IP address (first address in range).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert_eq!(ip_network.network_address(), Ipv4Addr::new(192, 168, 1, 0));
    /// ```
    #[inline]
    pub fn network_address(&self) -> Ipv4Addr {
        self.network_address
    }

    /// Returns broadcast address of network (last address in range).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert_eq!(ip_network.broadcast_address(), Ipv4Addr::new(192, 168, 1, 255));
    /// ```
    pub fn broadcast_address(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.network_address) | !helpers::get_bite_mask(self.netmask))
    }

    /// Returns network mask as integer.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert_eq!(ip_network.netmask(), 24);
    /// ```
    #[inline]
    pub fn netmask(&self) -> u8 {
        self.netmask
    }

    /// Returns network mask as IPv4 address.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert_eq!(ip_network.full_netmask(), Ipv4Addr::new(255, 255, 255, 0));
    /// ```
    pub fn full_netmask(&self) -> Ipv4Addr {
        Ipv4Addr::from(helpers::get_bite_mask(self.netmask))
    }

    /// Returns [`true`] if given [`IPv4Addr`] is inside this network.
    ///
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    /// [`Ipv4Addr`]: https://doc.rust-lang.org/std/net/struct.Ipv4Addr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert!(ip_network.contains(Ipv4Addr::new(192, 168, 1, 2)));
    /// assert!(!ip_network.contains(Ipv4Addr::new(192, 168, 2, 2)));
    /// ```
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        u32::from(ip) & helpers::get_bite_mask(self.netmask) == u32::from(self.network_address)
    }

    /// Returns iterator over host IP addresses in range (without network and broadcast address).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip = Ipv4Addr::new(192, 168, 1, 0);
    /// let mut hosts = Ipv4Network::new(ip, 24).unwrap().hosts();
    /// assert_eq!(hosts.next().unwrap(), Ipv4Addr::new(192, 168, 1, 1));
    /// assert_eq!(hosts.last().unwrap(), Ipv4Addr::new(192, 168, 1, 254));
    /// ```
    pub fn hosts(&self) -> iterator::Ipv4RangeIterator {
        let from = Ipv4Addr::from(u32::from(self.network_address).saturating_add(1));
        let to = Ipv4Addr::from(u32::from(self.broadcast_address()).saturating_sub(1));
        iterator::Ipv4RangeIterator::new(from, to)
    }

    /// Returns network with smaller netmask by one. If netmask is already zero, `None` will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip = Ipv4Addr::new(192, 168, 1, 0);
    /// let mut hosts = Ipv4Network::new(ip, 24).unwrap();
    /// assert_eq!(hosts.supernet(), Some(Ipv4Network::new(Ipv4Addr::new(192, 168, 0, 0), 23).unwrap()));
    /// ```
    pub fn supernet(&self) -> Option<Self> {
        if self.netmask == 0 {
            None
        } else {
            Some(Self::new_truncate(self.network_address, self.netmask - 1).unwrap())
        }
    }

    /// Returns `Ipv4NetworkIterator` over networks with bigger netmask by one.
    /// If netmask is already 32, `None` will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// let mut iterator = ip_network.subnets().unwrap();
    /// assert_eq!(iterator.next().unwrap(), Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 25).unwrap());
    /// assert_eq!(iterator.last().unwrap(), Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 128), 25).unwrap());
    /// ```
    pub fn subnets(&self) -> Option<iterator::Ipv4NetworkIterator> {
        if self.netmask == Self::LENGTH {
            None
        } else {
            Some(iterator::Ipv4NetworkIterator::new(self.clone(), self.netmask + 1))
        }
    }

    /// Returns `Ipv4NetworkIterator` over networks with defined netmask.
    ///
    /// # Panics
    ///
    /// This method panics when prefix is bigger than 32 or when prefix is lower or equal than netmask.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip = Ipv4Addr::new(192, 168, 1, 0);
    /// let mut iterator = Ipv4Network::new(ip, 24).unwrap().subnets_with_prefix(25);
    /// assert_eq!(iterator.next().unwrap(), Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 25).unwrap());
    /// assert_eq!(iterator.last().unwrap(), Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 128), 25).unwrap());
    /// ```
    pub fn subnets_with_prefix(&self, prefix: u8) -> iterator::Ipv4NetworkIterator {
        iterator::Ipv4NetworkIterator::new(self.clone(), prefix)
    }

    /// Returns [`true`] for the special 'unspecified' network (0.0.0.0/32).
    ///
    /// This property is defined in _UNIX Network Programming, Second Edition_,
    /// W. Richard Stevens, p. 891; see also [ip7].
    ///
    /// [ip7]: http://man7.org/linux/man-pages/man7/ip.7.html
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(0, 0, 0, 0), 32).unwrap();
    /// assert!(ip_network.is_unspecified());
    /// ```
    pub fn is_unspecified(&self) -> bool {
        u32::from(self.network_address) == 0 && self.netmask == Self::LENGTH
    }

    /// Returns [`true`] if this network is inside loopback address range (127.0.0.0/8).
    ///
    /// This property is defined by [IETF RFC 1122].
    ///
    /// [IETF RFC 1122]: https://tools.ietf.org/html/rfc1122
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(127, 0, 0, 0), 8).unwrap();
    /// assert!(ip_network.is_loopback());
    /// ```
    pub fn is_loopback(&self) -> bool {
        self.network_address.is_loopback()
    }

    /// Returns [`true`] if this is a broadcast network (255.255.255.255/32).
    ///
    /// A broadcast address has all octets set to 255 as defined in [IETF RFC 919].
    ///
    /// [IETF RFC 919]: https://tools.ietf.org/html/rfc919
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(255, 255, 255, 255), 32).unwrap();
    /// assert!(ip_network.is_broadcast());
    /// ```
    pub fn is_broadcast(&self) -> bool {
        self.network_address.is_broadcast()
    }

    /// Returns [`true`] if this whole network range is inside private address ranges.
    ///
    /// The private address ranges are defined in [IETF RFC 1918] and include:
    ///
    ///  - 10.0.0.0/8
    ///  - 172.16.0.0/12
    ///  - 192.168.0.0/16
    ///
    /// [IETF RFC 1918]: https://tools.ietf.org/html/rfc1918
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert!(ip_network.is_private());
    /// ```
    pub fn is_private(&self) -> bool {
        let octets = self.network_address.octets();
        match octets[0] {
            10 if self.netmask >= 8 => true,
            172 if octets[1] >= 16 && octets[1] <= 31 && self.netmask >= 12 => true,
            192 if octets[1] == 168 && self.netmask >= 16 => true,
            _ => false,
        }
    }

    /// Returns [`true`] if the network is is inside link-local range (169.254.0.0/16).
    ///
    /// This property is defined by [IETF RFC 3927].
    ///
    /// [IETF RFC 3927]: https://tools.ietf.org/html/rfc3927
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(169, 254, 1, 0), 24).unwrap();
    /// assert!(ip_network.is_link_local());
    /// ```
    pub fn is_link_local(&self) -> bool {
        let octets = self.network_address.octets();
        octets[0] == 169 && octets[1] == 254 && self.netmask >= 16
    }

    /// Returns [`true`] if this whole network is inside multicast address range (224.0.0.0/4).
    ///
    /// Multicast network addresses have a most significant octet between 224 and 239,
    /// and is defined by [IETF RFC 5771].
    ///
    /// [IETF RFC 5771]: https://tools.ietf.org/html/rfc5771
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(224, 168, 1, 0), 24).unwrap();
    /// assert!(ip_network.is_multicast());
    /// ```
    pub fn is_multicast(&self) -> bool {
        self.network_address.is_multicast() && self.netmask >= 4
    }

    /// Returns [`true`] if this network is in a range designated for documentation.
    ///
    /// This is defined in [IETF RFC 5737]:
    ///
    /// - 192.0.2.0/24 (TEST-NET-1)
    /// - 198.51.100.0/24 (TEST-NET-2)
    /// - 203.0.113.0/24 (TEST-NET-3)
    ///
    /// [IETF RFC 5737]: https://tools.ietf.org/html/rfc5737
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(192, 0, 2, 0), 24).unwrap();
    /// assert!(ip_network.is_documentation());
    /// ```
    pub fn is_documentation(&self) -> bool {
        self.network_address.is_documentation() && self.netmask >= 24
    }

    /// Returns [`true`] if the network appears to be globally routable.
    /// See [iana-ipv4-special-registry][ipv4-sr].
    ///
    /// The following return false:
    ///
    /// - private address (10.0.0.0/8, 172.16.0.0/12 and 192.168.0.0/16)
    /// - the loopback address (127.0.0.0/8)
    /// - the link-local address (169.254.0.0/16)
    /// - the broadcast address (255.255.255.255/32)
    /// - test addresses used for documentation (192.0.2.0/24, 198.51.100.0/24 and 203.0.113.0/24)
    /// - the unspecified address (0.0.0.0/32)
    ///
    /// [ipv4-sr]: https://www.iana.org/assignments/iana-ipv4-special-registry/iana-ipv4-special-registry.xhtml
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// assert!(!Ipv4Network::new(Ipv4Addr::new(10, 254, 0, 0), 16).unwrap().is_global());
    /// assert!(!Ipv4Network::new(Ipv4Addr::new(192, 168, 10, 65), 32).unwrap().is_global());
    /// assert!(!Ipv4Network::new(Ipv4Addr::new(172, 16, 10, 65), 32).unwrap().is_global());
    /// assert!(!Ipv4Network::new(Ipv4Addr::new(0, 0, 0, 0), 32).unwrap().is_global());
    /// assert!(Ipv4Network::new(Ipv4Addr::new(80, 9, 12, 3), 32).unwrap().is_global());
    /// ```
    pub fn is_global(&self) -> bool {
        !self.is_private()
            && !self.is_loopback()
            && !self.is_link_local()
            && !self.is_broadcast()
            && !self.is_documentation()
            && !self.is_unspecified()
    }

    // TODO: Documentation
    pub fn summarize_address_range(first: Ipv4Addr, last: Ipv4Addr) -> Vec<Self> {
        let mut first_int = u32::from(first);
        let last_int = u32::from(last);

        let mut vector = Vec::with_capacity(1);

        while first_int <= last_int {
            let bit_length_diff;
            if last_int - first_int == std::u32::MAX {
                bit_length_diff = Self::LENGTH;
            } else {
                bit_length_diff = helpers::bit_length(last_int - first_int + 1) - 1
            }

            let nbits = cmp::min(first_int.trailing_zeros() as u8, bit_length_diff);

            vector.push(Self::new(Ipv4Addr::from(first_int), Self::LENGTH - nbits).unwrap());

            if nbits == Self::LENGTH {
                break;
            }

            match first_int.checked_add(1 << nbits) {
                Some(x) => first_int = x,
                None => break,
            }
        }

        vector
    }
}

impl fmt::Display for Ipv4Network {
    /// Converts `Ipv4Network` to string in format X.X.X.X/Y (CIDR notation).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip_network = Ipv4Network::new(Ipv4Addr::new(192, 168, 1, 0), 24).unwrap();
    /// assert_eq!(ip_network.to_string(), "192.168.1.0/24");
    /// ```
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}/{}", self.network_address, self.netmask)
    }
}

impl FromStr for Ipv4Network {
    type Err = IpNetworkParseError;

    /// Converts string in format X.X.X.X/Y (CIDR notation) to `Ipv4Network`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    /// use std::str::FromStr;
    ///
    /// let ip_network = Ipv4Network::from_str("192.168.1.0/24").unwrap();
    /// assert_eq!(ip_network.network_address(), Ipv4Addr::new(192, 168, 1, 0));
    /// assert_eq!(ip_network.netmask(), 24);
    /// ```
    fn from_str(s: &str) -> Result<Ipv4Network, IpNetworkParseError> {
        let (ip, netmask) =
            helpers::split_ip_netmask(s).ok_or(IpNetworkParseError::InvalidFormatError)?;

        let network_address =
            Ipv4Addr::from_str(ip).map_err(|_| IpNetworkParseError::AddrParseError)?;
        let netmask =
            u8::from_str(netmask).map_err(|_| IpNetworkParseError::InvalidNetmaskFormat)?;

        Self::new(network_address, netmask).map_err(IpNetworkParseError::IpNetworkError)
    }
}

impl From<Ipv4Addr> for Ipv4Network {
    /// Converts `Ipv4Addr` to `Ipv4Network` with netmask 32.
    fn from(ip: Ipv4Addr) -> Self {
        Self {
            network_address: ip,
            netmask: Self::LENGTH,
        }
    }
}

impl IntoIterator for Ipv4Network {
    type Item = Ipv4Addr;
    type IntoIter = iterator::Ipv4RangeIterator;

    /// Returns iterator over all IP addresses in range including network and broadcast addresses.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use ip_network::Ipv4Network;
    ///
    /// let ip = Ipv4Addr::new(192, 168, 1, 0);
    /// let mut iter = Ipv4Network::new(ip, 24).unwrap().into_iter();
    /// assert_eq!(iter.next().unwrap(), Ipv4Addr::new(192, 168, 1, 0));
    /// assert_eq!(iter.next().unwrap(), Ipv4Addr::new(192, 168, 1, 1));
    /// assert_eq!(iter.last().unwrap(), Ipv4Addr::new(192, 168, 1, 255));
    /// ```
    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter::new(self.network_address, self.broadcast_address())
    }
}

/// IPv6 Network
#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Ipv6Network {
    network_address: Ipv6Addr,
    netmask: u8,
}

impl Ipv6Network {
    /// IPv4 address length in bits.
    const LENGTH: u8 = 128;

    /// Constructs new `Ipv6Network` based on [`Ipv6Addr`] and `netmask`.
    ///
    /// Returns error if netmask is bigger than 128 or if host bits are set in `network_address`.
    ///
    /// [`Ipv6Addr`]: https://doc.rust-lang.org/std/net/struct.Ipv6Addr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// let ip = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0);
    /// let ip_network = Ipv6Network::new(ip, 32).unwrap();
    /// assert_eq!(ip_network.network_address(), ip);
    /// assert_eq!(ip_network.netmask(), 32);
    /// ```
    pub fn new(network_address: Ipv6Addr, netmask: u8) -> Result<Self, IpNetworkError> {
        if netmask > Self::LENGTH {
            return Err(IpNetworkError::NetmaskError(netmask));
        }

        if u128::from(network_address).trailing_zeros() < (Self::LENGTH as u32 - netmask as u32) {
            return Err(IpNetworkError::HostBitsSet);
        }

        Ok(Self {
            network_address,
            netmask,
        })
    }

    /// Constructs new `Ipv6Network` based on [`Ipv6Addr`] and `netmask` with truncating host bits
    /// from given `network_address`.
    ///
    /// Returns error if netmask is bigger than 128.
    ///
    /// [`Ipv6Addr`]: https://doc.rust-lang.org/std/net/struct.Ipv6Addr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// let ip = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 1, 0, 0);
    /// let ip_network = Ipv6Network::new_truncate(ip, 32).unwrap();
    /// assert_eq!(ip_network.network_address(), Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0));
    /// assert_eq!(ip_network.netmask(), 32);
    /// ```
    pub fn new_truncate(network_address: Ipv6Addr, netmask: u8) -> Result<Self, IpNetworkError> {
        if netmask > Self::LENGTH {
            return Err(IpNetworkError::NetmaskError(netmask));
        }

        let network_address_u128 =
            u128::from(network_address) & helpers::get_bite_mask_u128(netmask);
        let network_address = Ipv6Addr::from(network_address_u128);

        Ok(Self {
            network_address,
            netmask,
        })
    }

    /// Returns network IP address (first address in range).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// let ip = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0);
    /// let ip_network = Ipv6Network::new(ip, 32).unwrap();
    /// assert_eq!(ip_network.network_address(), ip);
    /// ```
    #[inline]
    pub fn network_address(&self) -> Ipv6Addr {
        self.network_address
    }

    /// Returns network mask.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// let ip = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0);
    /// let ip_network = Ipv6Network::new(ip, 32).unwrap();
    /// assert_eq!(ip_network.netmask(), 32);
    /// ```
    #[inline]
    pub fn netmask(&self) -> u8 {
        self.netmask
    }

    /// Returns [`true`] if given [`IPv6Addr`] is inside this network.
    ///
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    /// [`Ipv6Addr`]: https://doc.rust-lang.org/std/net/struct.Ipv6Addr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// let ip_network = Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 64).unwrap();
    /// assert!(ip_network.contains(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)));
    /// assert!(!ip_network.contains(Ipv6Addr::new(0x2001, 0xdb9, 0, 0, 0, 0, 0, 0)));
    /// ```
    pub fn contains(&self, ip: Ipv6Addr) -> bool {
        let truncated_ip = u128::from(ip) & helpers::get_bite_mask_u128(self.netmask);
        truncated_ip == u128::from(self.network_address)
    }

    /// Returns network with smaller netmask by one. If netmask is already zero, `None` will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// let network = Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32).unwrap();
    /// assert_eq!(network.supernet(), Some(Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 31).unwrap()));
    /// ```
    pub fn supernet(&self) -> Option<Self> {
        if self.netmask == 0 {
            None
        } else {
            Some(Self::new_truncate(self.network_address, self.netmask - 1).unwrap())
        }
    }

    /// Returns `Ipv6NetworkIterator` over networks with netmask bigger one.
    /// If netmask is already 128, `None` will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// let ip_network = Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32).unwrap();
    /// let mut iterator = ip_network.subnets().unwrap();
    /// assert_eq!(iterator.next().unwrap(), Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 33).unwrap());
    /// assert_eq!(iterator.last().unwrap(), Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0x8000, 0, 0, 0, 0, 0), 33).unwrap());
    /// ```
    pub fn subnets(&self) -> Option<iterator::Ipv6NetworkIterator> {
        if self.netmask == Self::LENGTH {
            None
        } else {
            Some(iterator::Ipv6NetworkIterator::new(self.clone(), self.netmask + 1))
        }
    }

    /// Returns `Ipv6NetworkIterator` over networks with defined netmask.
    ///
    /// # Panics
    ///
    /// This method panics when prefix is bigger than 128 or when prefix is lower or equal than netmask.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// let network = Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32).unwrap();
    /// let mut iterator = network.subnets_with_prefix(33);
    /// assert_eq!(iterator.next().unwrap(), Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 33).unwrap());
    /// assert_eq!(iterator.last().unwrap(), Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0x8000, 0, 0, 0, 0, 0), 33).unwrap());
    /// ```
    pub fn subnets_with_prefix(&self, prefix: u8) -> iterator::Ipv6NetworkIterator {
        iterator::Ipv6NetworkIterator::new(self.clone(), prefix)
    }

    /// Returns [`true`] for the special 'unspecified' network (::/128).
    ///
    /// This property is defined in [IETF RFC 4291].
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// assert!(!Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff), 128).unwrap().is_unspecified());
    /// assert!(Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 128).unwrap().is_unspecified());
    /// ```
    pub fn is_unspecified(&self) -> bool {
        self.network_address.is_unspecified() && self.netmask == Self::LENGTH
    }

    /// Returns [`true`] if this is a loopback network (::1/128).
    ///
    /// This property is defined in [IETF RFC 4291].
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// assert!(Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0x1), 128).unwrap().is_loopback());
    /// assert!(!Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff), 128).unwrap().is_loopback());
    /// ```
    pub fn is_loopback(&self) -> bool {
        self.network_address.is_loopback()
    }

    /// Returns [`true`] if the address appears to be globally routable.
    ///
    /// The following return [`false`]:
    ///
    /// - the loopback network
    /// - link-local, site-local, and unique local unicast networks
    /// - interface-, link-, realm-, admin- and site-local multicast networks
    ///
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    /// [`false`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// assert!(Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff), 128).unwrap().is_global());
    /// assert!(!Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0x1), 128).unwrap().is_global());
    /// assert!(Ipv6Network::new(Ipv6Addr::new(0, 0, 0x1c9, 0, 0, 0xafc8, 0, 0x1), 128).unwrap().is_global());
    /// ```
    pub fn is_global(&self) -> bool {
        match self.multicast_scope() {
            Some(Ipv6MulticastScope::Global) => true,
            None => self.is_unicast_global(),
            _ => false,
        }
    }

    /// Returns [`true`] if this is a part of unique local network (fc00::/7).
    ///
    /// This property is defined in [IETF RFC 4193].
    ///
    /// [IETF RFC 4193]: https://tools.ietf.org/html/rfc4193
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// assert!(Ipv6Network::new(Ipv6Addr::new(0xfc02, 0, 0, 0, 0, 0, 0, 0), 16).unwrap().is_unique_local());
    /// assert!(!Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff), 128).unwrap().is_unique_local());
    /// ```
    pub fn is_unique_local(&self) -> bool {
        (self.network_address.segments()[0] & 0xfe00) == 0xfc00 && self.netmask >= 7
    }

    /// Returns [`true`] if the network is part of unicast and link-local (fe80::/10).
    ///
    /// This property is defined in [IETF RFC 4291].
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// assert!(Ipv6Network::new(Ipv6Addr::new(0xfe8a, 0, 0, 0, 0, 0, 0, 0), 16).unwrap().is_unicast_link_local());
    /// assert!(!Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff), 128).unwrap().is_unicast_link_local());
    /// ```
    pub fn is_unicast_link_local(&self) -> bool {
        (self.network_address.segments()[0] & 0xffc0) == 0xfe80 && self.netmask >= 10
    }

    /// Returns [`true`] if this is a deprecated unicast site-local network (fec0::/10).
    ///
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// assert!(Ipv6Network::new(Ipv6Addr::new(0xfec2, 0, 0, 0, 0, 0, 0, 0), 16).unwrap().is_unicast_site_local());
    /// assert!(!Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff), 128).unwrap().is_unicast_site_local());
    /// ```
    pub fn is_unicast_site_local(&self) -> bool {
        (self.network_address.segments()[0] & 0xffc0) == 0xfec0 && self.netmask >= 10
    }

    /// Returns [`true`] if this is a part of network reserved for documentation (2001:db8::/32).
    ///
    /// This property is defined in [IETF RFC 3849].
    ///
    /// [IETF RFC 3849]: https://tools.ietf.org/html/rfc3849
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// assert!(Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32).unwrap().is_documentation());
    /// assert!(!Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff), 128).unwrap().is_documentation());
    /// ```
    pub fn is_documentation(&self) -> bool {
        let segments = self.network_address.segments();
        segments[0] == 0x2001 && segments[1] == 0xdb8 && self.netmask >= 32
    }

    /// Returns [`true`] if the network is a globally routable unicast network.
    ///
    /// The following return [`false`]:
    ///
    /// - the loopback network
    /// - the link-local network
    /// - the (deprecated) site-local network
    /// - unique local network
    /// - the unspecified network
    /// - the network range reserved for documentation
    ///
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    /// [`false`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// assert!(!Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32).unwrap().is_unicast_global());
    /// assert!(Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff), 128).unwrap().is_unicast_global());
    /// ```
    pub fn is_unicast_global(&self) -> bool {
        !self.is_multicast()
            && !self.is_loopback()
            && !self.is_unicast_link_local()
            && !self.is_unicast_site_local()
            && !self.is_unique_local()
            && !self.is_unspecified()
            && !self.is_documentation()
    }

    /// Returns [`true`] if this is a part of multicast network (ff00::/8).
    ///
    /// This property is defined by [IETF RFC 4291].
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    /// [`true`]: https://doc.rust-lang.org/std/primitive.bool.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// assert!(Ipv6Network::new(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0), 8).unwrap().is_multicast());
    /// assert!(!Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff), 128).unwrap().is_multicast());
    /// ```
    pub fn is_multicast(&self) -> bool {
        (self.network_address.segments()[0] & 0xff00) == 0xff00 && self.netmask >= 8
    }

    /// Returns the network's multicast scope if the network is multicast.
    ///
    /// These scopes are defined in [IETF RFC 7346].
    ///
    /// [IETF RFC 7346]: https://tools.ietf.org/html/rfc7346
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::{Ipv6Network, Ipv6MulticastScope};
    ///
    /// assert_eq!(Ipv6Network::new(Ipv6Addr::new(0xff0e, 0, 0, 0, 0, 0, 0, 0), 32).unwrap().multicast_scope(),
    ///                              Some(Ipv6MulticastScope::Global));
    /// assert_eq!(Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff), 128).unwrap().multicast_scope(), None);
    /// ```
    pub fn multicast_scope(&self) -> Option<Ipv6MulticastScope> {
        if self.is_multicast() {
            match self.network_address.segments()[0] & 0x000f {
                1 => Some(Ipv6MulticastScope::InterfaceLocal),
                2 => Some(Ipv6MulticastScope::LinkLocal),
                3 => Some(Ipv6MulticastScope::RealmLocal),
                4 => Some(Ipv6MulticastScope::AdminLocal),
                5 => Some(Ipv6MulticastScope::SiteLocal),
                8 => Some(Ipv6MulticastScope::OrganizationLocal),
                14 => Some(Ipv6MulticastScope::Global),
                _ => None,
            }
        } else {
            None
        }
    }
}

impl fmt::Display for Ipv6Network {
    /// Converts `Ipv6Network` to string in format X:X::X/Y (CIDR notation).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    ///
    /// let ip_network = Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32).unwrap();
    /// assert_eq!(ip_network.to_string(), "2001:db8::/32");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.network_address, self.netmask)
    }
}

impl FromStr for Ipv6Network {
    type Err = IpNetworkParseError;

    /// Converts string in format X:X::X/Y (CIDR notation) to `Ipv6Network`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    /// use ip_network::Ipv6Network;
    /// use std::str::FromStr;
    ///
    /// let ip_network = Ipv6Network::from_str("2001:db8::/32").unwrap();
    /// assert_eq!(ip_network.network_address(), Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0));
    /// assert_eq!(ip_network.netmask(), 32);
    /// ```
    fn from_str(s: &str) -> Result<Ipv6Network, IpNetworkParseError> {
        let (ip, netmask) =
            helpers::split_ip_netmask(s).ok_or(IpNetworkParseError::InvalidFormatError)?;

        let network_address =
            Ipv6Addr::from_str(ip).map_err(|_| IpNetworkParseError::AddrParseError)?;
        let netmask =
            u8::from_str(netmask).map_err(|_| IpNetworkParseError::InvalidNetmaskFormat)?;

        Self::new(network_address, netmask).map_err(IpNetworkParseError::IpNetworkError)
    }
}

impl From<Ipv6Addr> for Ipv6Network {
    /// Converts `Ipv6Addr` to `Ipv6Network` with netmask 128.
    fn from(ip: Ipv6Addr) -> Self {
        Self {
            network_address: ip,
            netmask: Self::LENGTH,
        }
    }
}

/// Errors when creating new IPv4 or IPv6 networks
#[derive(Debug)]
pub enum IpNetworkError {
    /// Network mask is bigger than possible for given IP version (32 for IPv4, 128 for IPv6)
    NetmaskError(u8),
    /// Host bits are set in given network IP address
    HostBitsSet,
}

impl Error for IpNetworkError {}

impl fmt::Display for IpNetworkError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let description = match *self {
            IpNetworkError::NetmaskError(_) => "invalid netmask",
            IpNetworkError::HostBitsSet => "IP network address has host bits set",
        };
        write!(fmt, "{}", description)
    }
}

/// Errors from IPv4 or IPv6 network parsing
#[derive(Debug)]
pub enum IpNetworkParseError {
    /// Network mask is not valid integer between 0-255
    InvalidNetmaskFormat,
    /// Network address has invalid format (not X/Y)
    InvalidFormatError,
    /// Invalid IP address syntax (IPv4 or IPv6)
    AddrParseError,
    /// Error when creating new IPv4 or IPv6 networks
    IpNetworkError(IpNetworkError),
}

impl Error for IpNetworkParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            IpNetworkParseError::IpNetworkError(ref ip_network_error) => Some(ip_network_error),
            _ => None,
        }
    }
}

impl fmt::Display for IpNetworkParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IpNetworkParseError::InvalidNetmaskFormat => write!(fmt, "invalid netmask format"),
            IpNetworkParseError::InvalidFormatError => write!(fmt, "invalid format"),
            IpNetworkParseError::AddrParseError => write!(fmt, "invalid IP address syntax"),
            IpNetworkParseError::IpNetworkError(ref ip_network_error) => {
                write!(fmt, "{}", ip_network_error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};
    use crate::{IpNetwork, IpNetworkError, IpNetworkParseError, Ipv4Network, Ipv6Network};

    fn return_test_ipv4_network() -> Ipv4Network {
        Ipv4Network::new(Ipv4Addr::new(192, 168, 0, 0), 16).unwrap()
    }

    fn return_test_ipv6_network() -> Ipv6Network {
        Ipv6Network::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32).unwrap()
    }

    #[test]
    fn ip_network_is_ipv4() {
        let ip_network = IpNetwork::V4(return_test_ipv4_network());
        assert!(ip_network.is_ipv4());
        assert!(!ip_network.is_ipv6());
    }

    #[test]
    fn ip_network_is_ipv6() {
        let ip_network = IpNetwork::V6(return_test_ipv6_network());
        assert!(ip_network.is_ipv6());
        assert!(!ip_network.is_ipv4());
    }

    #[test]
    fn ip_network_parse_ipv4() {
        let ip_network: IpNetwork = "192.168.0.0/16".parse().unwrap();
        assert_eq!(ip_network, IpNetwork::V4(return_test_ipv4_network()));
    }

    #[test]
    fn ip_network_parse_ipv6() {
        let ip_network: IpNetwork = "2001:db8::/32".parse().unwrap();
        assert_eq!(ip_network, IpNetwork::V6(return_test_ipv6_network()));
    }

    #[test]
    fn ip_network_parse_empty() {
        let ip_network = "".parse::<IpNetwork>();
        assert!(ip_network.is_err());
        assert!(match ip_network.err().unwrap() {
            IpNetworkParseError::InvalidFormatError => true,
            _ => false,
        });
    }

    #[test]
    fn ip_network_parse_invalid_netmask() {
        let ip_network = "192.168.0.0/a".parse::<IpNetwork>();
        assert!(ip_network.is_err());
        assert!(match ip_network.err().unwrap() {
            IpNetworkParseError::InvalidNetmaskFormat => true,
            _ => false,
        });
    }

    #[test]
    fn ip_network_parse_invalid_ip() {
        let ip_network = "192.168.0.0a/16".parse::<IpNetwork>();
        assert!(ip_network.is_err());
        assert!(match ip_network.err().unwrap() {
            IpNetworkParseError::AddrParseError => true,
            _ => false,
        });
    }

    #[test]
    fn ip_network_parse_ipv4_host_bits_set() {
        let ip_network = "192.168.0.1/16".parse::<IpNetwork>();
        assert!(ip_network.is_err());
        assert!(match ip_network.err().unwrap() {
            IpNetworkParseError::IpNetworkError(_) => true,
            _ => false,
        });
    }

    #[test]
    fn ip_network_parse_ipv6_host_bits_set() {
        let ip_network = "2001:db8::1/32".parse::<IpNetwork>();
        assert!(ip_network.is_err());
        assert!(match ip_network.err().unwrap() {
            IpNetworkParseError::IpNetworkError(_) => true,
            _ => false,
        });
    }

    #[test]
    fn ip_network_format_ipv4() {
        let ip_network = IpNetwork::V4(return_test_ipv4_network());
        assert_eq!(ip_network.to_string(), "192.168.0.0/16");
    }

    #[test]
    fn ip_network_format_ipv6() {
        let ip_network = IpNetwork::V6(return_test_ipv6_network());
        assert_eq!(ip_network.to_string(), "2001:db8::/32");
    }

    #[test]
    fn ipv4_network_new_host_bits_set() {
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let ip_network = Ipv4Network::new(ip, 8);
        assert!(ip_network.is_err());
        assert!(match ip_network.err().unwrap() {
            IpNetworkError::HostBitsSet => true,
            _ => false,
        });
    }

    #[test]
    fn ipv4_network_new_host_bits_set_no_31() {
        let ip = Ipv4Addr::new(127, 0, 0, 2);
        let ip_network = Ipv4Network::new(ip, 31);
        assert!(ip_network.is_ok());
    }

    #[test]
    fn ipv4_network_new_host_bits_set_no_32() {
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let ip_network = Ipv4Network::new(ip, 32);
        assert!(ip_network.is_ok());
    }

    #[test]
    fn ipv4_network_new_host_bits_set_no_zero() {
        let ip = Ipv4Addr::new(0, 0, 0, 0);
        let ip_network = Ipv4Network::new(ip, 0);
        assert!(ip_network.is_ok());
    }

    #[test]
    fn ipv4_network_new_big_invalid_netmask() {
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let ip_network = Ipv4Network::new(ip, 35);
        assert!(ip_network.is_err());
        assert!(match ip_network.err().unwrap() {
            IpNetworkError::NetmaskError(_) => true,
            _ => false,
        });
    }

    #[test]
    fn ipv4_network_new_truncate_host_bits_set() {
        let ip = Ipv4Addr::new(127, 0, 0, 1);
        let ip_network = Ipv4Network::new_truncate(ip, 8).unwrap();
        assert_eq!(ip_network.network_address(), Ipv4Addr::new(127, 0, 0, 0));
    }

    #[test]
    fn ipv4_network_basic_getters() {
        let ip_network = return_test_ipv4_network();
        assert_eq!(ip_network.network_address(), Ipv4Addr::new(192, 168, 0, 0));
        assert_eq!(ip_network.netmask(), 16);
        assert_eq!(
            ip_network.broadcast_address(),
            Ipv4Addr::new(192, 168, 255, 255)
        );
        assert_eq!(ip_network.full_netmask(), Ipv4Addr::new(255, 255, 0, 0));
        assert_eq!(
            ip_network.supernet(),
            Some(Ipv4Network::new(Ipv4Addr::new(192, 168, 0, 0), 15).unwrap())
        );
        assert_eq!(ip_network.hosts().len(), 256 * 256 - 2);
    }

    #[test]
    fn ipv4_network_iterator() {
        let ip_network = return_test_ipv4_network();
        assert_eq!(ip_network.into_iter().len(), 256 * 256);
    }

    #[test]
    fn ipv4_network_iterator_for() {
        let mut i = 0;
        for _ in return_test_ipv4_network() {
            i += 1;
        }
        assert_eq!(i, 256 * 256);
    }

    #[test]
    fn ipv4_network_contains() {
        let ip_network = return_test_ipv4_network();
        assert!(!ip_network.contains(Ipv4Addr::new(192, 167, 255, 255)));
        assert!(ip_network.contains(Ipv4Addr::new(192, 168, 0, 0)));
        assert!(ip_network.contains(Ipv4Addr::new(192, 168, 255, 255)));
        assert!(!ip_network.contains(Ipv4Addr::new(192, 169, 0, 0)));
    }

    #[test]
    fn ipv4_network_subnets() {
        let ip_network = return_test_ipv4_network();
        let mut subnets = ip_network.subnets().unwrap();
        assert_eq!(subnets.len(), 2);
        assert_eq!(
            subnets.next().unwrap(),
            Ipv4Network::new(Ipv4Addr::new(192, 168, 0, 0), 17).unwrap()
        );
        assert_eq!(
            subnets.next().unwrap(),
            Ipv4Network::new(Ipv4Addr::new(192, 168, 128, 0), 17).unwrap()
        );
        assert!(subnets.next().is_none());
    }

    #[test]
    fn ipv4_network_subnets_with_prefix() {
        let ip_network = return_test_ipv4_network();
        let mut subnets = ip_network.subnets_with_prefix(18);
        assert_eq!(subnets.len(), 4);
        assert_eq!(
            subnets.next().unwrap(),
            Ipv4Network::new(Ipv4Addr::new(192, 168, 0, 0), 18).unwrap()
        );
        assert_eq!(
            subnets.next().unwrap(),
            Ipv4Network::new(Ipv4Addr::new(192, 168, 64, 0), 18).unwrap()
        );
        assert_eq!(
            subnets.next().unwrap(),
            Ipv4Network::new(Ipv4Addr::new(192, 168, 128, 0), 18).unwrap()
        );
        assert_eq!(
            subnets.next().unwrap(),
            Ipv4Network::new(Ipv4Addr::new(192, 168, 192, 0), 18).unwrap()
        );
        assert!(subnets.next().is_none());
    }

    #[test]
    fn ipv4_network_parse() {
        let ip_network: Ipv4Network = "192.168.0.0/16".parse().unwrap();
        assert_eq!(ip_network, return_test_ipv4_network());
    }

    #[test]
    fn ipv4_network_format() {
        let ip_network = return_test_ipv4_network();
        assert_eq!(ip_network.to_string(), "192.168.0.0/16");
    }

    #[test]
    fn ipv4_network_cmd_different_ip() {
        let a = Ipv4Network::new(Ipv4Addr::new(127, 0, 0, 0), 8).unwrap();
        let b = Ipv4Network::new(Ipv4Addr::new(128, 0, 0, 0), 8).unwrap();
        assert!(b > a);
    }

    #[test]
    fn ipv4_network_cmd_different_netmask() {
        let a = Ipv4Network::new(Ipv4Addr::new(127, 0, 0, 0), 8).unwrap();
        let b = Ipv4Network::new(Ipv4Addr::new(127, 0, 0, 0), 16).unwrap();
        assert!(b > a);
    }

    #[test]
    fn ipv4_network_is_private() {
        let is_private = |ip, netmask| Ipv4Network::new(ip, netmask).unwrap().is_private();

        assert!(is_private(Ipv4Addr::new(10, 0, 0, 0), 8));
        assert!(!is_private(Ipv4Addr::new(10, 0, 0, 0), 7));
        assert!(is_private(Ipv4Addr::new(10, 0, 0, 0), 32));
        assert!(!is_private(Ipv4Addr::new(11, 0, 0, 0), 32));

        assert!(is_private(Ipv4Addr::new(172, 16, 0, 0), 12));
        assert!(is_private(Ipv4Addr::new(172, 16, 0, 0), 32));
        assert!(is_private(Ipv4Addr::new(172, 31, 255, 255), 32));
        assert!(!is_private(Ipv4Addr::new(172, 32, 0, 0), 32));

        assert!(is_private(Ipv4Addr::new(192, 168, 0, 0), 16));
        assert!(is_private(Ipv4Addr::new(192, 168, 0, 0), 32));
        assert!(!is_private(Ipv4Addr::new(192, 168, 0, 0), 15));
    }

    #[test]
    fn ipv4_network_is_global() {
        let is_global = |ip, netmask| Ipv4Network::new(ip, netmask).unwrap().is_global();

        assert!(!is_global(Ipv4Addr::new(10, 0, 0, 0), 8));
        assert!(is_global(Ipv4Addr::new(10, 0, 0, 0), 7));
        assert!(!is_global(Ipv4Addr::new(10, 0, 0, 0), 32));
        assert!(is_global(Ipv4Addr::new(11, 0, 0, 0), 32));

        assert!(!is_global(Ipv4Addr::new(172, 16, 0, 0), 12));
        assert!(!is_global(Ipv4Addr::new(172, 16, 0, 0), 32));
        assert!(!is_global(Ipv4Addr::new(172, 31, 255, 255), 32));
        assert!(is_global(Ipv4Addr::new(172, 32, 0, 0), 32));

        assert!(!is_global(Ipv4Addr::new(192, 168, 0, 0), 16));
        assert!(!is_global(Ipv4Addr::new(192, 168, 0, 0), 32));
        assert!(is_global(Ipv4Addr::new(192, 168, 0, 0), 15));

        assert!(!is_global(Ipv4Addr::new(127, 0, 0, 0), 8));
        assert!(!is_global(Ipv4Addr::new(169, 254, 0, 0), 16));
        assert!(!is_global(Ipv4Addr::new(255, 255, 255, 255), 32));
        assert!(!is_global(Ipv4Addr::new(192, 0, 2, 0), 24));
        assert!(!is_global(Ipv4Addr::new(198, 51, 100, 0), 24));
        assert!(!is_global(Ipv4Addr::new(203, 0, 113, 0), 24));
    }

    #[test]
    fn ipv4_network_hashmap() {
        use std::collections::HashMap;

        let ip = Ipv4Addr::new(127, 0, 0, 0);
        let network = Ipv4Network::new(ip, 8).unwrap();

        let mut networks = HashMap::new();
        networks.insert(network, 256);

        let ip_contains = Ipv4Addr::new(127, 0, 0, 0);
        let network_contains = Ipv4Network::new(ip_contains, 8).unwrap();
        assert!(networks.contains_key(&network_contains));

        let ip_not_contains = Ipv4Addr::new(127, 0, 0, 0);
        let network_not_contains = Ipv4Network::new(ip_not_contains, 9).unwrap();
        assert!(!networks.contains_key(&network_not_contains));
    }

    #[test]
    fn ipv4_network_summarize_address_range() {
        let networks = Ipv4Network::summarize_address_range(
            Ipv4Addr::new(194, 249, 198, 0),
            Ipv4Addr::new(194, 249, 198, 159),
        );
        assert_eq!(networks.len(), 2);
        assert_eq!(
            networks[0],
            Ipv4Network::new(Ipv4Addr::new(194, 249, 198, 0), 25).unwrap()
        );
        assert_eq!(
            networks[1],
            Ipv4Network::new(Ipv4Addr::new(194, 249, 198, 128), 27).unwrap()
        );
    }

    #[test]
    fn ipv4_network_summarize_address_range_whole_range() {
        let networks = Ipv4Network::summarize_address_range(
            Ipv4Addr::new(0, 0, 0, 0),
            Ipv4Addr::new(255, 255, 255, 255),
        );
        assert_eq!(networks.len(), 1);
        assert_eq!(
            networks[0],
            Ipv4Network::new(Ipv4Addr::new(0, 0, 0, 0), 0).unwrap()
        );
    }

    #[test]
    fn ipv6_network_new() {
        let ip = Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0);
        let network = Ipv6Network::new(ip, 7).unwrap();
        assert_eq!(
            network.network_address(),
            Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0)
        );
        assert_eq!(network.netmask(), 7);
    }

    #[test]
    fn ipv6_network_contains() {
        let ip_network = return_test_ipv6_network();
        assert!(!ip_network.contains(Ipv6Addr::new(
            0x2001, 0x0db7, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff
        )));
        assert!(ip_network.contains(Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0)));
        assert!(ip_network.contains(Ipv6Addr::new(
            0x2001, 0x0db8, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff
        )));
        assert!(!ip_network.contains(Ipv6Addr::new(0x2001, 0x0db9, 0, 0, 0, 0, 0, 0)));
    }

    #[test]
    fn ipv6_network_supernet() {
        let ip_network = return_test_ipv6_network();
        assert_eq!(
            ip_network.supernet(),
            Some(Ipv6Network::new(Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0), 31).unwrap())
        );
    }

    #[test]
    fn ipv6_network_subnets() {
        let mut subnets = return_test_ipv6_network().subnets().unwrap();
        assert_eq!(subnets.len(), 2);
        assert_eq!(
            subnets.next().unwrap(),
            Ipv6Network::new(Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0), 33).unwrap()
        );
        assert_eq!(
            subnets.next().unwrap(),
            Ipv6Network::new(Ipv6Addr::new(0x2001, 0x0db8, 0x8000, 0, 0, 0, 0, 0), 33).unwrap()
        );
        assert!(subnets.next().is_none());
    }

    #[test]
    fn ipv6_network_subnets_with_prefix() {
        let ip_network = return_test_ipv6_network();
        let mut subnets = ip_network.subnets_with_prefix(34);
        assert_eq!(subnets.len(), 4);
        assert_eq!(
            subnets.next().unwrap(),
            Ipv6Network::new(Ipv6Addr::new(0x2001, 0x0db8, 0x0000, 0, 0, 0, 0, 0), 34).unwrap()
        );
        assert_eq!(
            subnets.next().unwrap(),
            Ipv6Network::new(Ipv6Addr::new(0x2001, 0x0db8, 0x4000, 0, 0, 0, 0, 0), 34).unwrap()
        );
        assert_eq!(
            subnets.next().unwrap(),
            Ipv6Network::new(Ipv6Addr::new(0x2001, 0x0db8, 0x8000, 0, 0, 0, 0, 0), 34).unwrap()
        );
        assert_eq!(
            subnets.next().unwrap(),
            Ipv6Network::new(Ipv6Addr::new(0x2001, 0x0db8, 0xc000, 0, 0, 0, 0, 0), 34).unwrap()
        );
        assert!(subnets.next().is_none());
    }

    #[test]
    fn ipv6_network_parse() {
        let ip_network: Ipv6Network = "2001:db8::/32".parse().unwrap();
        assert_eq!(ip_network, return_test_ipv6_network());
    }

    #[test]
    fn ipv6_network_format() {
        let ip_network = return_test_ipv6_network();
        assert_eq!(ip_network.to_string(), "2001:db8::/32");
    }
}
