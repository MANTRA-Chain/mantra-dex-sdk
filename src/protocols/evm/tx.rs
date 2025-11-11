// Allow deprecated Signature for compatibility with alloy-consensus ecosystem
#![allow(deprecated)]

#[cfg(feature = "evm")]
use alloy_consensus::{SignableTransaction, Signed, TxEip1559};
#[cfg(feature = "evm")]
use alloy_eips::eip2930::AccessList;
#[cfg(feature = "evm")]
use alloy_primitives::{Address, Bytes, ChainId, Signature, TxKind, B256, U256};

/// Convenience builder for constructing and signing EIP-1559 transactions.
#[cfg(feature = "evm")]
#[derive(Clone, Debug)]
pub struct Eip1559Transaction {
    pub chain_id: ChainId,
    pub nonce: u64,
    pub gas_limit: u64,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub to: Option<Address>,
    pub value: U256,
    pub data: Bytes,
    pub access_list: AccessList,
}

#[cfg(feature = "evm")]
impl Eip1559Transaction {
    /// Create a new transaction with default zeroed value/data/access list.
    pub fn new(chain_id: u64, nonce: u64) -> Self {
        Self {
            chain_id,
            nonce,
            gas_limit: 21_000,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            to: None,
            value: U256::ZERO,
            data: Bytes::new(),
            access_list: AccessList::default(),
        }
    }

    /// Set the target address (None implies contract creation).
    pub fn to(mut self, to: Option<Address>) -> Self {
        self.to = to;
        self
    }

    /// Set the value (in wei) to transfer.
    pub fn value(mut self, value: U256) -> Self {
        self.value = value;
        self
    }

    /// Set the calldata payload.
    pub fn data(mut self, data: Bytes) -> Self {
        self.data = data;
        self
    }

    /// Set the gas limit for the transaction.
    pub fn gas_limit(mut self, gas_limit: u64) -> Self {
        self.gas_limit = gas_limit;
        self
    }

    /// Set the max fee per gas (wei).
    pub fn max_fee_per_gas(mut self, max_fee: u128) -> Self {
        self.max_fee_per_gas = max_fee;
        self
    }

    /// Set the priority fee per gas (tip) in wei.
    pub fn max_priority_fee_per_gas(mut self, tip: u128) -> Self {
        self.max_priority_fee_per_gas = tip;
        self
    }

    /// Provide a pre-built access list.
    pub fn access_list(mut self, list: AccessList) -> Self {
        self.access_list = list;
        self
    }

    fn to_kind(&self) -> TxKind {
        self.to.map(TxKind::Call).unwrap_or_else(|| TxKind::Create)
    }

    fn to_alloy(&self) -> TxEip1559 {
        TxEip1559 {
            chain_id: self.chain_id,
            nonce: self.nonce,
            gas_limit: self.gas_limit,
            max_fee_per_gas: self.max_fee_per_gas,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas,
            to: self.to_kind(),
            value: self.value,
            access_list: self.access_list.clone(),
            input: self.data.clone(),
        }
    }

    fn into_alloy(self) -> TxEip1559 {
        TxEip1559 {
            chain_id: self.chain_id,
            nonce: self.nonce,
            gas_limit: self.gas_limit,
            max_fee_per_gas: self.max_fee_per_gas,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas,
            to: self.to_kind(),
            value: self.value,
            access_list: self.access_list,
            input: self.data,
        }
    }

    /// Bytes that should be hashed (keccak256) for signing.
    pub fn encoded_for_signing(&self) -> Vec<u8> {
        self.to_alloy().encoded_for_signing()
    }

    /// Convenience helper to compute signature hash (keccak256) of the transaction.
    pub fn signature_hash(&self) -> B256 {
        self.to_alloy().signature_hash()
    }

    /// Encode the signed transaction into raw bytes suitable for submission.
    pub fn encode_signed(&self, signature: &Signature) -> Bytes {
        let tx = self.to_alloy();
        let mut buf = Vec::with_capacity(tx.encoded_len_with_signature(signature, false));
        tx.encode_with_signature(signature, &mut buf, false);
        Bytes::from(buf)
    }

    /// Consume the builder and combine with signature producing a Signed tx.
    pub fn into_signed(self, signature: Signature) -> Signed<TxEip1559> {
        self.into_alloy().into_signed(signature)
    }
}

/// Wrapper containing the fully signed transaction and raw payload.
#[cfg(feature = "evm")]
#[derive(Clone, Debug)]
pub struct SignedEip1559Transaction {
    signed: Signed<TxEip1559>,
    raw: Bytes,
}

#[cfg(feature = "evm")]
impl SignedEip1559Transaction {
    pub fn new(signed: Signed<TxEip1559>, raw: Bytes) -> Self {
        Self { signed, raw }
    }

    /// Raw bytes ready to be sent via `eth_sendRawTransaction`.
    pub fn raw(&self) -> &Bytes {
        &self.raw
    }

    /// Transaction hash computed from the signed payload.
    pub fn hash(&self) -> B256 {
        *self.signed.hash()
    }

    /// Access signature data (v, r, s).
    pub fn signature(&self) -> &Signature {
        self.signed.signature()
    }

    /// Borrow the inner signed transaction structure.
    pub fn as_signed(&self) -> &Signed<TxEip1559> {
        &self.signed
    }

    /// Consume wrapper and return raw bytes.
    pub fn into_raw(self) -> Bytes {
        self.raw
    }
}
