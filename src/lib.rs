//! Miden RPC client with native miden_objects types
use miden_objects::{
    account::AccountId,
    note::{NoteId, NoteTag},
    utils::Serializable,
    Word,
};
use tonic::{Request, transport::{Channel, ClientTlsConfig}};

// Re-export proto types for advanced usage
pub use miden_node_proto::generated::{
    account, block_producer, blockchain, note, primitives, rpc, rpc_store, shared, transaction,
};
pub use rpc::api_client::ApiClient;

// Conversion helpers
mod convert {
    use super::*;

    /// Convert Word to proto Digest
    pub fn word_to_digest(word: Word) -> primitives::Digest {
        primitives::Digest {
            d0: word[0].as_int(),
            d1: word[1].as_int(),
            d2: word[2].as_int(),
            d3: word[3].as_int(),
        }
    }

    /// Convert NoteId to proto Digest
    pub fn note_id_to_digest(note_id: NoteId) -> primitives::Digest {
        word_to_digest(note_id.as_word())
    }

    /// Convert AccountId to proto AccountId
    pub fn account_id_to_proto(account_id: &AccountId) -> account::AccountId {
        account::AccountId {
            id: account_id.to_bytes().to_vec(),
        }
    }
}

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
    /// Takes AccountId and NoteTag types from miden_objects
    pub async fn sync_state(
        &mut self,
        block_num: u32,
        account_ids: &[AccountId],
        note_tags: &[NoteTag],
    ) -> Result<rpc_store::SyncStateResponse, String> {
        let account_ids = account_ids
            .iter()
            .map(|id| convert::account_id_to_proto(id))
            .collect();

        let note_tags = note_tags.iter().map(|tag| tag.as_u32()).collect();

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
    /// Takes Word types representing nullifier digests
    pub async fn check_nullifiers(
        &mut self,
        nullifiers: &[Word],
    ) -> Result<rpc_store::CheckNullifiersResponse, String> {
        let nullifiers = nullifiers
            .iter()
            .map(|w| convert::word_to_digest(*w))
            .collect();
        let request = rpc_store::NullifierList { nullifiers };

        let response = self
            .client
            .check_nullifiers(Request::new(request))
            .await
            .map_err(|e| format!("CheckNullifiers RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Get notes by their IDs
    /// Takes NoteId types from miden_objects
    pub async fn get_notes_by_id(
        &mut self,
        note_ids: &[NoteId],
    ) -> Result<note::CommittedNoteList, String> {
        let note_ids = note_ids
            .iter()
            .map(|id| note::NoteId {
                id: Some(convert::note_id_to_digest(*id)),
            })
            .collect();
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

    /// Get account proofs for the specified accounts
    /// Returns state proofs for multiple accounts with optional storage requests
    /// Takes Word types for code commitments
    pub async fn get_account_proofs(
        &mut self,
        account_requests: Vec<rpc_store::account_proofs_request::AccountRequest>,
        include_headers: bool,
        code_commitments: &[Word],
    ) -> Result<rpc_store::AccountProofs, String> {
        let code_commitments = code_commitments
            .iter()
            .map(|w| convert::word_to_digest(*w))
            .collect();

        let request = rpc_store::AccountProofsRequest {
            account_requests,
            include_headers: Some(include_headers),
            code_commitments,
        };

        let response = self
            .client
            .get_account_proofs(Request::new(request))
            .await
            .map_err(|e| format!("GetAccountProofs RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Get raw block data by block number
    pub async fn get_block_by_number(
        &mut self,
        block_num: u32,
    ) -> Result<blockchain::MaybeBlock, String> {
        let request = blockchain::BlockNumber { block_num };

        let response = self
            .client
            .get_block_by_number(Request::new(request))
            .await
            .map_err(|e| format!("GetBlockByNumber RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Submit a proven batch of transactions to the network
    /// The batch is provided as a single encoded byte vector
    pub async fn submit_proven_batch(
        &mut self,
        encoded_batch: Vec<u8>,
    ) -> Result<block_producer::SubmitProvenBatchResponse, String> {
        let request = transaction::ProvenTransactionBatch {
            encoded: encoded_batch,
        };

        let response = self
            .client
            .submit_proven_batch(Request::new(request))
            .await
            .map_err(|e| format!("SubmitProvenBatch RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Check nullifiers by prefixes (only 16-bit prefixes are supported)
    /// Returns a list of nullifiers that match the specified prefixes
    pub async fn check_nullifiers_by_prefix(
        &mut self,
        prefix_len: u32,
        nullifiers: Vec<u32>,
        block_num: u32,
    ) -> Result<rpc_store::CheckNullifiersByPrefixResponse, String> {
        let request = rpc_store::CheckNullifiersByPrefixRequest {
            prefix_len,
            nullifiers,
            block_num,
        };

        let response = self
            .client
            .check_nullifiers_by_prefix(Request::new(request))
            .await
            .map_err(|e| format!("CheckNullifiersByPrefix RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Sync account vault updates within a block range
    /// Takes AccountId from miden_objects
    pub async fn sync_account_vault(
        &mut self,
        account_id: &AccountId,
        block_from: u32,
        block_to: Option<u32>,
    ) -> Result<rpc_store::SyncAccountVaultResponse, String> {
        let request = rpc_store::SyncAccountVaultRequest {
            account_id: Some(convert::account_id_to_proto(account_id)),
            block_from,
            block_to,
        };

        let response = self
            .client
            .sync_account_vault(Request::new(request))
            .await
            .map_err(|e| format!("SyncAccountVault RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Sync notes by note tags and block height
    /// Takes NoteTag types from miden_objects
    pub async fn sync_notes(
        &mut self,
        block_num: u32,
        note_tags: &[NoteTag],
    ) -> Result<rpc_store::SyncNotesResponse, String> {
        let note_tags = note_tags.iter().map(|tag| tag.as_u32()).collect();

        let request = rpc_store::SyncNotesRequest {
            block_num,
            note_tags,
        };

        let response = self
            .client
            .sync_notes(Request::new(request))
            .await
            .map_err(|e| format!("SyncNotes RPC failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Sync storage map updates for specified account within a block range
    /// Takes AccountId from miden_objects
    pub async fn sync_storage_maps(
        &mut self,
        account_id: &AccountId,
        block_from: u32,
        block_to: Option<u32>,
    ) -> Result<rpc_store::SyncStorageMapsResponse, String> {
        let request = rpc_store::SyncStorageMapsRequest {
            account_id: Some(convert::account_id_to_proto(account_id)),
            block_from,
            block_to,
        };

        let response = self
            .client
            .sync_storage_maps(Request::new(request))
            .await
            .map_err(|e| format!("SyncStorageMaps RPC failed: {}", e))?;

        Ok(response.into_inner())
    }
}
