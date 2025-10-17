//! Minimal Miden RPC client using miden-node-proto crate
//!
//! This crate provides a simple wrapper around the Miden node RPC API client,
//! using the official miden-node-proto crate instead of maintaining local proto files.

use miden_objects::{account::AccountId, utils::Serializable};
use tonic::{Request,transport::{Channel, ClientTlsConfig}};

// Re-export all proto modules from miden-node-proto
pub use miden_node_proto::generated::{
    account, block_producer, blockchain, note, primitives, rpc, rpc_store, shared, transaction,
};
pub use rpc::api_client::ApiClient;

/// Simple wrapper around the tonic-generated ApiClient
pub struct MidenRpcClient {
    client: ApiClient<Channel>,
}

impl MidenRpcClient {
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, String> {
        let endpoint_str = endpoint.into();

        let channel = Channel::from_shared(endpoint_str.clone())
            .map_err(|e| format!("Invalid endpoint: {}", e))?
            .tls_config(ClientTlsConfig::new().with_native_roots())
            .map_err(|e| format!("TLS config error: {}", e))?
            .connect()
            .await
            .map_err(|e| format!("Failed to connect to {}: {}", endpoint_str, e))?;

        let client = ApiClient::new(channel);

        Ok(Self { client })
    }

    /// Get the underlying tonic ApiClient for full access to all RPC methods:
    pub fn client_mut(&mut self) -> &mut ApiClient<Channel> {
        &mut self.client
    }

    /// Get the status of the Miden node
    pub async fn get_status(&mut self) -> Result<rpc::RpcStatus, String> {
        let response = self
            .client
            .status(Request::new(()))
            .await
            .map_err(|e| format!("Status RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Get block header by number with optional MMR proof
    pub async fn get_block_header(
        &mut self,
        block_num: Option<u32>,
        include_mmr_proof: bool,
    ) -> Result<shared::BlockHeaderByNumberResponse, String> {
        let request = shared::BlockHeaderByNumberRequest {
            block_num,
            include_mmr_proof: Some(include_mmr_proof),
        };

        let response = self
            .client
            .get_block_header_by_number(Request::new(request))
            .await
            .map_err(|e| format!("GetBlockHeaderByNumber RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Submit a proven transaction to the network
    pub async fn submit_transaction(
        &mut self,
        proven_tx_bytes: Vec<u8>,
    ) -> Result<block_producer::SubmitProvenTransactionResponse, String> {
        let request = transaction::ProvenTransaction {
            transaction: proven_tx_bytes,
        };

        let response = self
            .client
            .submit_proven_transaction(Request::new(request))
            .await
            .map_err(|e| format!("SubmitProvenTransaction RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Sync state for specified accounts and note tags
    pub async fn sync_state(
        &mut self,
        block_num: u32,
        account_ids: Vec<Vec<u8>>,
        note_tags: Vec<u32>,
    ) -> Result<rpc_store::SyncStateResponse, String> {
        let account_ids = account_ids.into_iter().map(|id| account::AccountId { id }).collect();

        let request = rpc_store::SyncStateRequest {
            block_num,
            account_ids,
            note_tags,
        };

        let response = self
            .client
            .sync_state(Request::new(request))
            .await
            .map_err(|e| format!("SyncState RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Check nullifiers and get their proofs
    pub async fn check_nullifiers(
        &mut self,
        nullifiers: Vec<primitives::Digest>,
    ) -> Result<rpc_store::CheckNullifiersResponse, String> {
        let request = rpc_store::NullifierList { nullifiers };

        let response = self
            .client
            .check_nullifiers(Request::new(request))
            .await
            .map_err(|e| format!("CheckNullifiers RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Get notes by their IDs
    pub async fn get_notes_by_id(
        &mut self,
        note_ids: Vec<primitives::Digest>,
    ) -> Result<note::CommittedNoteList, String> {
        let note_ids = note_ids.into_iter().map(|id| note::NoteId { id: Some(id) }).collect();
        let request = note::NoteIdList { ids: note_ids };

        let response = self
            .client
            .get_notes_by_id(Request::new(request))
            .await
            .map_err(|e| format!("GetNotesById RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Fetch account commitment from the Miden network
    pub async fn get_account_commitment(
        &mut self,
        account_id: &AccountId,
    ) -> Result<String, String> {
        let account_id_bytes = account_id.to_bytes();

        let request = Request::new(account::AccountId {
            id: account_id_bytes.to_vec(),
        });

        let response = self
            .client
            .get_account_details(request)
            .await
            .map_err(|e| format!("RPC call failed: {}", e))?;

        let account_details = response.into_inner();

        let summary = account_details
            .summary
            .ok_or_else(|| "No account summary in response".to_string())?;

        let commitment = summary
            .account_commitment
            .ok_or_else(|| "No commitment in account summary".to_string())?;

        // Convert Digest to hex string
        let bytes = [
            commitment.d0.to_le_bytes(),
            commitment.d1.to_le_bytes(),
            commitment.d2.to_le_bytes(),
            commitment.d3.to_le_bytes(),
        ].concat();

        Ok(format!("0x{}", hex::encode(bytes)))
    }

    /// Fetch full account details including serialized account data
    pub async fn get_account_details(
        &mut self,
        account_id: &AccountId,
    ) -> Result<account::AccountDetails, String> {
        let account_id_bytes = account_id.to_bytes();

        let request = Request::new(account::AccountId {
            id: account_id_bytes.to_vec(),
        });

        let response = self
            .client
            .get_account_details(request)
            .await
            .map_err(|e| format!("RPC call failed: {}", e))?;

        Ok(response.into_inner())
    }
}
