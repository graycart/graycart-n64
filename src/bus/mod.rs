//! Physical memory bus shell (full decode table, RDRAM 9th-bit side array,
//! SysAD quirks, and ISViewer land in Task M0.3).
//!
//! Cited: N64brew — Memory map
//!   https://n64brew.dev/wiki/Memory_map · docs/02-bus-memory-dma.md

use serde::{Deserialize, Serialize};

use crate::ai::Ai;
use crate::cart::Cart;
use crate::mi::Mi;
use crate::pi::Pi;
use crate::pif::Pif;
use crate::ri::Ri;
use crate::si::Si;
use crate::vi::Vi;

/// CPU cacheability attribute passed from MMU/TLB to the SysAD bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheAttr {
    Cached,
    Uncached,
}

/// Physical memory region classification (full table wired in M0.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysRegion {
    Rdram,
    UnmappedFreeze,
}

/// Placeholder RDRAM wrapper (9th-bit side array lands in M0.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rdram {
    data: Vec<u8>,
}

impl Rdram {
    pub const BASE_SIZE: usize = 4 * 1024 * 1024;
    pub const EXPANSION_SIZE: usize = 8 * 1024 * 1024;

    #[must_use]
    pub fn new(expansion_pak: bool) -> Self {
        let size = if expansion_pak {
            Self::EXPANSION_SIZE
        } else {
            Self::BASE_SIZE
        };
        Self {
            data: vec![0; size],
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

/// Physical bus owner (sole memory authority).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bus {
    pub rdram: Rdram,
    pub mi: Mi,
    pub vi: Vi,
    pub ai: Ai,
    pub pi: Pi,
    pub ri: Ri,
    pub si: Si,
    pub pif: Pif,
    pub cart: Option<Cart>,
    pub isviewer_enabled: bool,
}

impl Bus {
    #[must_use]
    pub fn new(expansion_pak: bool, isviewer_enabled: bool) -> Self {
        Self {
            rdram: Rdram::new(expansion_pak),
            mi: Mi::new(),
            vi: Vi::new(),
            ai: Ai::new(),
            pi: Pi::new(),
            ri: Ri::new(),
            si: Si::new(),
            pif: Pif::new(),
            cart: None,
            isviewer_enabled,
        }
    }

    pub fn attach_cart(&mut self, cart: Cart) {
        self.cart = Some(cart);
    }

    pub fn copy_cart_soft_boot(&mut self) {}

    #[must_use]
    pub fn isviewer_output(&self) -> &[u8] {
        &[]
    }
}
