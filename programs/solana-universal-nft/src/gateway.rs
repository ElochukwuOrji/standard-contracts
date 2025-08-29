// src/gateway.rs
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    secp256k1_recover::{secp256k1_recover, Secp256k1Pubkey},
    keccak,
};
use anchor_spl::{token, associated_token};

/// ZetaChain Gateway Program constants (converted from base58)
pub const GATEWAY_PROGRAM_ID: Pubkey = anchor_lang::solana_program::pubkey!("ZETAjseVjuFsxdRxo6MmTCvqFwb3ZHUx56Co3vCmGis");

pub const GATEWAY_PDA: Pubkey = anchor_lang::solana_program::pubkey!("2f9SLuUNb7TNeM6gzBwT4ZjbL5ZyKzzHg1Ce9yiquEjj");

/// Gateway instruction discriminators (from ZetaChain Gateway IDL)
pub mod instruction_discriminators {
    // These are the actual discriminators from ZetaChain Gateway program
    pub const DEPOSIT: [u8; 8] = [242, 35, 198, 137, 82, 225, 242, 182];
    pub const DEPOSIT_SPL_TOKEN: [u8; 8] = [134, 180, 1, 126, 157, 176, 47, 0];
    pub const WITHDRAW: [u8; 8] = [183, 18, 70, 156, 148, 109, 161, 34];
}

/// Gateway state account structure
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct GatewayState {
    pub discriminator: [u8; 8],
    pub authority: Pubkey,
    pub tss_address: [u8; 20],
    pub nonce: u64,
}

/// Revert context for gateway calls
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct RevertContext {
    pub asset: Pubkey,
    pub amount: u64,
    pub revert_message: Vec<u8>,
}

/// Create a deposit instruction for the ZetaChain Gateway
pub fn create_deposit_instruction(
    receiver: [u8; 20],
    amount: u64,
    asset: Option<Pubkey>, // None for SOL, Some(mint) for SPL tokens
    revert_context: Option<RevertContext>,
    payer: Pubkey,
) -> Result<Instruction> {
    
    let mut accounts = vec![
        AccountMeta::new(GATEWAY_PDA, false),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
    ];

    let mut instruction_data = if asset.is_some() {
        // SPL token deposit
        instruction_discriminators::DEPOSIT_SPL_TOKEN.to_vec()
    } else {
        // SOL deposit
        instruction_discriminators::DEPOSIT.to_vec()
    };

    // Serialize the instruction parameters
    let mut params = vec![];
    params.extend_from_slice(&receiver);
    params.extend_from_slice(&amount.to_le_bytes());
    
    // Add asset if SPL token
    if let Some(mint) = asset {
        params.push(1); // Some
        params.extend_from_slice(&mint.to_bytes());
        
        // Add token accounts for SPL
        let gateway_token_account = associated_token::get_associated_token_address(
            &GATEWAY_PDA,
            &mint
        );
        let payer_token_account = associated_token::get_associated_token_address(
            &payer,
            &mint
        );
        
        accounts.extend(vec![
            AccountMeta::new(gateway_token_account, false),
            AccountMeta::new(payer_token_account, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(token::ID, false),
            AccountMeta::new_readonly(associated_token::ID, false),
        ]);
    } else {
        params.push(0); // None
    }
    
    // Add revert context
    if let Some(revert) = revert_context {
        params.push(1); // Some
        params.extend_from_slice(&revert.try_to_vec().unwrap());
    } else {
        params.push(0); // None
    }
    
    instruction_data.extend_from_slice(&params);

    Ok(Instruction {
        program_id: GATEWAY_PROGRAM_ID,
        accounts,
        data: instruction_data,
    })
}

/// Create a withdraw instruction for the ZetaChain Gateway
/// This is called by the ZetaChain TSS to send assets from gateway to users
pub fn create_withdraw_instruction(
    nonce: u64,
    amount: u64,
    receiver: Pubkey,
    asset: Option<Pubkey>, // None for SOL, Some(mint) for SPL tokens
    signature: [u8; 64],
    recovery_id: u8,
    message_hash: [u8; 32],
    payer: Pubkey,
) -> Result<Instruction> {
    
    let mut accounts = vec![
        AccountMeta::new(GATEWAY_PDA, false),
        AccountMeta::new(receiver, false),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(system_program::ID, false),
    ];

    let mut instruction_data = instruction_discriminators::WITHDRAW.to_vec();
    
    // Serialize parameters
    let mut params = vec![];
    params.extend_from_slice(&nonce.to_le_bytes());
    params.extend_from_slice(&amount.to_le_bytes());
    
    // Add asset if SPL token
    if let Some(mint) = asset {
        params.push(1); // Some
        params.extend_from_slice(&mint.to_bytes());
        
        // Add token accounts
        let gateway_token_account = associated_token::get_associated_token_address(
            &GATEWAY_PDA,
            &mint
        );
        let receiver_token_account = associated_token::get_associated_token_address(
            &receiver,
            &mint
        );
        
        accounts.extend(vec![
            AccountMeta::new(gateway_token_account, false),
            AccountMeta::new(receiver_token_account, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(token::ID, false),
            AccountMeta::new_readonly(associated_token::ID, false),
        ]);
    } else {
        params.push(0); // None
    }
    
    // Add signature data
    params.extend_from_slice(&signature);
    params.push(recovery_id);
    params.extend_from_slice(&message_hash);
    
    instruction_data.extend_from_slice(&params);

    Ok(Instruction {
        program_id: GATEWAY_PROGRAM_ID,
        accounts,
        data: instruction_data,
    })
}

/// Helper to get the Gateway PDA
pub fn get_gateway_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"meta"], &GATEWAY_PROGRAM_ID)
}

/// Helper to validate TSS signature
pub fn validate_tss_signature(
    signature: [u8; 64],
    recovery_id: u8,
    message_hash: [u8; 32],
    expected_tss_address: [u8; 20],
) -> Result<bool> {
    // Recover the public key from signature
    let recovered_pubkey = secp256k1_recover(&message_hash, recovery_id, &signature)
        .map_err(|_| error!(crate::ErrorCode::InvalidTssSignature))?;
    
    // Convert to Ethereum address format
    let recovered_address = pubkey_to_eth_address(&recovered_pubkey);
    
    Ok(recovered_address == expected_tss_address)
}

/// Convert Solana pubkey to Ethereum address format
fn pubkey_to_eth_address(pubkey: &Secp256k1Pubkey) -> [u8; 20] {
    // Get the uncompressed public key (64 bytes)
    let pubkey_bytes = &pubkey.to_bytes();
    
    // Hash with Keccak256
    let hash = keccak::hash(pubkey_bytes);
    
    // Take last 20 bytes for Ethereum address
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash.to_bytes()[12..]);
    address
}