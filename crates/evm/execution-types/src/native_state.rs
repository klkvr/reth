//! Flat owned native changes for asynchronous execution hooks.

use alloc::vec::Vec;
use alloy_primitives::{Address, B256};
use evm2::{
    bytecode::Bytecode,
    evm::{AccountChangeRef, AccountInfo, StateChangeSink, StateChangeSource, StorageChange},
};

/// Flat transaction updates. Block accumulation consumes the original source directly.
pub type EvmState = Vec<StateChange>;

/// One owned callback from an evm2 transaction's change stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateChange {
    /// Changed code needed by consumers retaining block transitions.
    Bytecode(B256, Bytecode),
    /// Clear storage before applying subsequent slot writes.
    StorageWipe(Address),
    /// A slot's original and current values.
    Storage(StorageChange),
    /// Account metadata and lifecycle at the transaction boundary.
    Account {
        /// Changed address.
        address: Address,
        /// Original info, without bytecode.
        original: Option<AccountInfo>,
        /// Current info, without bytecode.
        current: Option<AccountInfo>,
        /// Whether the transaction created the account.
        created: bool,
        /// Whether it selfdestructed the account.
        selfdestructed: bool,
    },
}

impl StateChange {
    /// Owns account metadata without retaining embedded code.
    pub fn account(change: AccountChangeRef<'_>) -> Self {
        fn info(info: &AccountInfo) -> AccountInfo {
            let mut info = info.clone();
            info.code = None;
            info
        }
        Self::Account {
            address: change.address,
            original: change.original.map(info),
            current: change.current.map(info),
            created: change.created,
            selfdestructed: change.selfdestructed,
        }
    }
}

/// Borrows flat updates as an evm2 change source for diagnostic consumers.
#[derive(Debug)]
pub struct StateChanges<'a>(pub &'a [StateChange]);
impl StateChangeSource for StateChanges<'_> {
    fn visit<S: StateChangeSink>(&self, sink: &mut S) -> Result<(), S::Error> {
        for change in self.0 {
            match change {
                StateChange::Bytecode(hash, code) => sink.bytecode(*hash, code)?,
                StateChange::StorageWipe(address) => sink.storage_wipe(*address)?,
                StateChange::Storage(change) => sink.storage(*change)?,
                StateChange::Account { address, original, current, created, selfdestructed } => {
                    sink.account(AccountChangeRef {
                        address: *address,
                        original: original.as_ref(),
                        current: current.as_ref(),
                        created: *created,
                        selfdestructed: *selfdestructed,
                    })?
                }
            }
        }
        Ok(())
    }
}

/// Receives finalized native transaction updates.
pub trait OnStateHook {
    /// Processes one transaction's flat changes.
    fn on_state(&mut self, state: EvmState);
}
impl<F: FnMut(EvmState)> OnStateHook for F {
    fn on_state(&mut self, state: EvmState) {
        self(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockState, TransactionChanges};
    use alloy_primitives::U256;
    use evm2::evm::PendingState;
    #[test]
    fn flat_hook_matches_direct_block_and_legacy_bundle() {
        let address = Address::with_last_byte(1);
        let mut direct = BlockState::new();
        let mut replay = BlockState::new();
        let mut legacy = BlockState::new();
        for (original, current) in [(5, 7), (7, 9), (9, 5)] {
            let mut pending = PendingState::default();
            pending.insert_account(
                address,
                Some(AccountInfo::empty().with_nonce(1)),
                Some(AccountInfo::empty().with_nonce(1)),
            );
            pending.insert_account(
                Address::with_last_byte(2),
                Some(AccountInfo::empty()),
                Some(AccountInfo::empty()),
            );
            pending.insert_storage(address, U256::ZERO, U256::from(original), U256::from(current));
            pending.insert_storage(address, U256::from(1), U256::from(2), U256::from(2));
            let mut updates = Vec::new();
            let Ok(()) = pending.visit(&mut direct.transaction_sink(Some(&mut updates)));
            assert_eq!(updates.len(), 2, "one changed slot and its account metadata; no reads");
            replay.commit(&StateChanges(&updates));
            let mut changes = TransactionChanges::default();
            let Ok(()) = pending.visit(&mut changes);
            legacy.commit_revm(&changes);
        }
        let mut direct = direct.into_bundle();
        let mut replay = replay.into_bundle();
        let mut legacy = legacy.into_bundle();
        assert_eq!(direct, legacy);
        assert_eq!(replay, legacy);
        assert!(direct.revert_latest());
        assert!(replay.revert_latest());
        assert!(legacy.revert_latest());
        assert_eq!(direct, legacy);
        assert_eq!(replay, legacy);
    }
}
