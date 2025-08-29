use anchor_client::{
    solana_sdk::{
        pubkey::Pubkey,
        signature::{Keypair, Signature, Signer},
        system_instruction,
        transaction::Transaction,
    },
    Client, Cluster, Program,
};
use anchor_lang::prelude::*;
use solana_universal_nft::{
    accounts, instruction, NftCollection, CrossChainState, TransferStatus,
    RevertOptions, NftTransferData,
};
use std::rc::Rc;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, UniversalNftError>;

#[derive(Error, Debug)]
pub enum UniversalNftError {
    #[error("Anchor client error: {0}")]
    AnchorClient(#[from] anchor_client::ClientError),
    #[error("Program error: {0}")]
    Program(#[from] anchor_lang::error::Error),
    #[error("Invalid token ID")]
    InvalidTokenId,
    #[error("NFT not found")]
    NftNotFound,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Cross-chain transfer failed")]
    CrossChainTransferFailed,
}

pub struct UniversalNftClient {
    program: Program<Rc<Keypair>>,
    program_id: Pubkey,
}

impl UniversalNftClient {
    pub fn new(cluster: Cluster, payer: Rc<Keypair>, program_id: Option<Pubkey>) -> Self {
        let client = Client::new(cluster, payer);
        let program_id = program_id.unwrap_or(solana_universal_nft::ID);
        let program = client.program(program_id);

        Self { program, program_id }
    }

    /// Initialize a new NFT collection
    pub async fn initialize_collection(
        &self,
        name: String,
        symbol: String,
        base_uri: String,
    ) -> Result<Signature> {
        let (collection_pda, _) = Pubkey::find_program_address(
            &[b"nft_collection"],
            &self.program_id,
        );

        let tx = self
            .program
            .request()
            .accounts(accounts::Initialize {
                authority: self.program.payer(),
                nft_collection: collection_pda,
                system_program: solana_program::system_program::id(),
            })
            .args(instruction::Initialize {
                name,
                symbol,
                base_uri,
            })
            .send()
            .await?;

        Ok(tx)
    }

    /// Mint a new NFT
    pub async fn mint_nft(
        &self,
        token_id: u64,
        name: String,
        uri: String,
        creator_fee: u16,
        owner: Option<Pubkey>,
    ) -> Result<Signature> {
        let owner_pubkey = owner.unwrap_or(self.program.payer());

        let (collection_pda, _) = Pubkey::find_program_address(
            &[b"nft_collection"],
            &self.program_id,
        );

        let (authority_pda, _) = Pubkey::find_program_address(
            &[b"authority"],
            &self.program_id,
        );

        let (nft_mint_pda, _) = Pubkey::find_program_address(
            &[b"nft_mint", &token_id.to_le_bytes()],
            &self.program_id,
        );

        let owner_token_account = spl_associated_token_account::get_associated_token_address(
            &owner_pubkey,
            &nft_mint_pda,
        );

        let (nft_metadata_pda, _) = Pubkey::find_program_address(
            &[
                b"metadata",
                mpl_token_metadata::ID.as_ref(),
                nft_mint_pda.as_ref(),
            ],
            &mpl_token_metadata::ID,
        );

        let tx = self
            .program
            .request()
            .accounts(accounts::MintNft {
                owner: owner_pubkey,
                nft_collection: collection_pda,
                authority: authority_pda,
                nft_mint: nft_mint_pda,
                nft_token_account: owner_token_account,
                nft_metadata: nft_metadata_pda,
                token_program: spl_token::id(),
                metadata_program: mpl_token_metadata::ID,
                system_program: solana_program::system_program::id(),
                rent: solana_program::sysvar::rent::id(),
            })
            .args(instruction::MintNft {
                token_id,
                name,
                uri,
                creator_fee,
            })
            .send()
            .await?;

        Ok(tx)
    }

    /// Send NFT to another blockchain via ZetaChain
    pub async fn send_nft_cross_chain(
        &self,
        token_id: u64,
        destination_chain_id: u64,
        destination_address: [u8; 20],
        owner: Option<Pubkey>,
        revert_options: Option<RevertOptions>,
    ) -> Result<Signature> {
        let owner_pubkey = owner.unwrap_or(self.program.payer());

        let (authority_pda, _) = Pubkey::find_program_address(
            &[b"authority"],
            &self.program_id,
        );

        let (nft_mint_pda, _) = Pubkey::find_program_address(
            &[b"nft_mint", &token_id.to_le_bytes()],
            &self.program_id,
        );

        let owner_token_account = spl_associated_token_account::get_associated_token_address(
            &owner_pubkey,
            &nft_mint_pda,
        );

        let program_token_account = spl_associated_token_account::get_associated_token_address(
            &authority_pda,
            &nft_mint_pda,
        );

        let (cross_chain_state_pda, _) = Pubkey::find_program_address(
            &[b"cross_chain_state", &token_id.to_le_bytes()],
            &self.program_id,
        );

        let (nft_metadata_pda, _) = Pubkey::find_program_address(
            &[
                b"metadata",
                mpl_token_metadata::ID.as_ref(),
                nft_mint_pda.as_ref(),
            ],
            &mpl_token_metadata::ID,
        );

        // ZetaChain Gateway program and state
        let gateway_program = Pubkey::from_str("ZETAjseVjuFsxdRxo6MmTCvqFwb3ZHUx56Co3vCmGis").unwrap();
        let gateway_state = Pubkey::new_unique();

        let tx = self
            .program
            .request()
            .accounts(accounts::SendNftCrossChain {
                owner: owner_pubkey,
                authority: authority_pda,
                nft_token_account: owner_token_account,
                program_nft_account: program_token_account,
                nft_mint: nft_mint_pda,
                nft_metadata: nft_metadata_pda,
                cross_chain_state: cross_chain_state_pda,
                gateway_program,
                gateway_state,
                token_program: spl_token::id(),
                metadata_program: mpl_token_metadata::ID,
                system_program: solana_program::system_program::id(),
            })
            .args(instruction::SendNftCrossChain {
                token_id,
                destination_chain_id,
                destination_address,
                revert_options,
            })
            .send()
            .await?;

        Ok(tx)
    }

    /// Get collection information
    pub async fn get_collection(&self) -> Result<NftCollection> {
        let (collection_pda, _) = Pubkey::find_program_address(
            &[b"nft_collection"],
            &self.program_id,
        );

        let account = self.program.account::<NftCollection>(collection_pda).await?;
        Ok(account)
    }

    /// Get cross-chain transfer status
    pub async fn get_cross_chain_status(&self, token_id: u64) -> Result<CrossChainState> {
        let (cross_chain_state_pda, _) = Pubkey::find_program_address(
            &[b"cross_chain_state", &token_id.to_le_bytes()],
            &self.program_id,
        );

        let account = self.program.account::<CrossChainState>(cross_chain_state_pda).await?;
        Ok(account)
    }

    /// Get NFT mint address for a token ID
    pub fn get_nft_mint_address(&self, token_id: u64) -> Pubkey {
        let (mint_pda, _) = Pubkey::find_program_address(
            &[b"nft_mint", &token_id.to_le_bytes()],
            &self.program_id,
        );
        mint_pda
    }

    /// Get NFT metadata address for a token ID
    pub fn get_nft_metadata_address(&self, token_id: u64) -> Pubkey {
        let mint_pda = self.get_nft_mint_address(token_id);
        let (metadata_pda, _) = Pubkey::find_program_address(
            &[
                b"metadata",
                mpl_token_metadata::ID.as_ref(),
                mint_pda.as_ref(),
            ],
            &mpl_token_metadata::ID,
        );
        metadata_pda
    }

    /// Get token account address for an NFT and owner
    pub fn get_token_account_address(&self, token_id: u64, owner: Pubkey) -> Pubkey {
        let mint_pda = self.get_nft_mint_address(token_id);
        spl_associated_token_account::get_associated_token_address(&owner, &mint_pda)
    }

    /// Update NFT metadata (authority only)
    pub async fn update_nft_metadata(
        &self,
        token_id: u64,
        new_uri: Option<String>,
        new_name: Option<String>,
    ) -> Result<Signature> {
        let (authority_pda, _) = Pubkey::find_program_address(
            &[b"authority"],
            &self.program_id,
        );

        let nft_mint = self.get_nft_mint_address(token_id);
        let nft_metadata = self.get_nft_metadata_address(token_id);

        let tx = self
            .program
            .request()
            .accounts(accounts::UpdateNftMetadata {
                authority: self.program.payer(),
                program_authority: authority_pda,
                nft_mint,
                nft_metadata,
                metadata_program: mpl_token_metadata::ID,
            })
            .args(instruction::UpdateNftMetadata {
                token_id,
                new_uri,
                new_name,
            })
            .send()
            .await?;

        Ok(tx)
    }
}

/// Helper struct for building NFT transfer data
#[derive(Clone)]
pub struct NftTransferBuilder {
    pub token_id: u64,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub creator_fee: u16,
}

impl NftTransferBuilder {
    pub fn new(token_id: u64) -> Self {
        Self {
            token_id,
            name: String::new(),
            symbol: String::new(),
            uri: String::new(),
            creator_fee: 0,
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn symbol(mut self, symbol: String) -> Self {
        self.symbol = symbol;
        self
    }

    pub fn uri(mut self, uri: String) -> Self {
        self.uri = uri;
        self
    }

    pub fn creator_fee(mut self, creator_fee: u16) -> Self {
        self.creator_fee = creator_fee;
        self
    }

    pub fn build(self) -> NftTransferData {
        NftTransferData {
            token_id: self.token_id,
            name: self.name,
            symbol: self.symbol,
            uri: self.uri,
            creator_fee: self.creator_fee,
        }
    }
}

/// Constants for chain IDs
pub mod chain_ids {
    pub const ETHEREUM: u64 = 1;
    pub const BSC: u64 = 56;
    pub const POLYGON: u64 = 137;
    pub const AVALANCHE: u64 = 43114;
    pub const ARBITRUM: u64 = 42161;
    pub const OPTIMISM: u64 = 10;
    pub const ZETACHAIN: u64 = 7000;
}

/// Utility functions for address conversion
pub mod utils {
    use solana_program::pubkey::Pubkey;
    
    /// Convert Solana pubkey to 20-byte Ethereum-style address
    pub fn pubkey_to_eth_address(pubkey: &Pubkey) -> [u8; 20] {
        let mut address = [0u8; 20];
        address.copy_from_slice(&pubkey.to_bytes()[..20]);
        address
    }
    
    /// Convert 20-byte Ethereum-style address to string
    pub fn eth_address_to_string(address: &[u8; 20]) -> String {
        format!("0x{}", hex::encode(address))
    }
    
    /// Parse Ethereum-style address string to bytes
    pub fn parse_eth_address(address: &str) -> Result<[u8; 20], hex::FromHexError> {
        let address = address.strip_prefix("0x").unwrap_or(address);
        let bytes = hex::decode(address)?;
        if bytes.len() != 20 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut result = [0u8; 20];
        result.copy_from_slice(&bytes);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_client::Cluster;
    use solana_sdk::signature::Keypair;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_client_initialization() {
        let payer = Rc::new(Keypair::new());
        let client = UniversalNftClient::new(
            Cluster::Localnet,
            payer,
            None,
        );
        
        assert_eq!(client.program_id, solana_universal_nft::ID);
    }

    #[test]
    fn test_nft_transfer_builder() {
        let transfer_data = NftTransferBuilder::new(123)
            .name("Test NFT".to_string())
            .symbol("TEST".to_string())
            .uri("https://example.com/123".to_string())
            .creator_fee(500)
            .build();

        assert_eq!(transfer_data.token_id, 123);
        assert_eq!(transfer_data.name, "Test NFT");
        assert_eq!(transfer_data.symbol, "TEST");
        assert_eq!(transfer_data.uri, "https://example.com/123");
        assert_eq!(transfer_data.creator_fee, 500);
    }

    #[test]
    fn test_address_conversion() {
        let pubkey = Pubkey::new_unique();
        let eth_address = utils::pubkey_to_eth_address(&pubkey);
        let address_string = utils::eth_address_to_string(&eth_address);
        
        assert!(address_string.starts_with("0x"));
        assert_eq!(address_string.len(), 42); // 0x + 40 hex chars
        
        let parsed_address = utils::parse_eth_address(&address_string).unwrap();
        assert_eq!(parsed_address, eth_address);
    }
}