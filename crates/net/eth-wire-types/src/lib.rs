//! Types for the eth wire protocol: <https://github.com/ethereum/devp2p/blob/master/caps/eth.md>

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/paradigmxyz/reth/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod status;
use alloy_rlp::{Decodable, Encodable};
use reth_consensus::ConsensusError;
use reth_primitives::{
    alloy_primitives::{Sealable, Sealed}, Header, SealedBlock
};
pub use status::{Status, StatusBuilder};

pub mod version;
pub use version::{EthVersion, ProtocolVersion};

pub mod message;
pub use message::{EthMessage, EthMessageID, ProtocolMessage};

pub mod header;
pub use header::*;

pub mod blocks;
pub use blocks::*;

pub mod broadcast;
pub use broadcast::*;

pub mod transactions;
pub use transactions::*;

pub mod state;
pub use state::*;

pub mod receipts;
pub use receipts::*;

pub mod disconnect_reason;
pub use disconnect_reason::*;

pub mod capability;
pub use capability::*;

pub trait NetworkTypes: Send + Sync + core::fmt::Debug + Clone + Unpin + 'static {
    type Transaction;
    type Block: Block<Header = Self::BlockHeader, Body = Self::BlockBody>;
    type BlockBody: BlockBody;
    type BlockHeader: BlockHeader;
    type Receipt;

    fn validate_block_body(
        header: &Self::BlockHeader,
        body: &Self::BlockBody,
    ) -> Result<(), ConsensusError>;
}

pub trait BlockHeader:
    Encodable
    + Decodable
    + Send
    + Sync
    + core::fmt::Debug
    + Clone
    + Unpin
    + Sealable
    + PartialEq
    + Eq
    + core::hash::Hash
    + Into<Header>
    + From<Header>
{
    fn number(&self) -> u64;
}

pub trait BlockBody:
    Encodable + Decodable + Send + Sync + core::fmt::Debug + Clone + Unpin + From<reth_primitives::BlockBody>
{
}

pub trait Block: Encodable + Decodable + Send + Sync + core::fmt::Debug + Clone + Unpin {
    type Header: BlockHeader;
    type Body: BlockBody;

    fn new(header: Self::Header, body: Self::Body) -> Self;
    fn build_sealed(header: Sealed<Self::Header>, body: Self::Body) -> SealedBlock;
    fn split_sealed(block: SealedBlock) -> (Sealed<Self::Header>, Self::Body);

    fn header(&self) -> &Self::Header;
    fn body(&self) -> &Self::Body;

    fn split(self) -> (Self::Header, Self::Body);
}
