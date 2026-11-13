//! The lookup of the address space of a bot.
//!
//! The address space of a bot is the set of prefixes it published. The lookup
//! answers whether an address falls into that set.

use std::net::IpAddr;

use ipnet::IpNet;

/// Whether the address falls into one of the prefixes.
pub fn covers(prefixes: &[IpNet], ip: IpAddr) -> bool {
    prefixes.iter().any(|net| net.contains(&ip))
}

/// The address of the first prefix — the witness of the set.
pub fn witness(prefixes: &[IpNet]) -> Option<IpAddr> {
    prefixes.first().map(|net| net.network())
}
