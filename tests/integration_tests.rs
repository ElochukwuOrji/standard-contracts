use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint};
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
    pubkey::Pubkey,
    system_instruction,
    rent::Rent,
};
use spl_token::instruction as token_instruction;
use std::str::FromStr;

use solana_universal_nft::{
    instruction as program_instruction,
    state::*,
    ErrorCode,
    ID as PROGRAM_ID,
};

#[tokio::test]
async fn test_initialize_collection() {
    let program_test = ProgramTest::new(
        "solana_universal_nft",
        PROGRAM_ID,
        processor!(solana_universal_nft::entry),
    );
    
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Derive collection PDA
    let (collection_pda, _) = Pubkey::find_program_address(
        &[b"nft_collection"],
        &PROGRAM_ID,
    );

    let initialize_ix = program_instruction::Initialize {
        name: "Universal NFT Collection".to_string(),
        symbol: "UNC".to_string(),
        base_uri: "https://api.example.com/metadata/".to_string(),
    };

    let accounts = solana_universal_nft::accounts::Initialize {
        authority: payer.pubkey(),
        nft_collection: collection_pda,
        system_program: solana_program::system_program::id(),
    };

    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: accounts.to_account_metas(None),
        data: initialize_ix.data(),
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Initialize collection failed: {:?}", result);

    // Verify collection state
    let collection_account = banks_client
        .get_account(collection_pda)
        .await
        .expect("Failed to get collection account")
        .expect("Collection account not found");

    let collection_data = NftCollection::try_deserialize(
        &mut &collection_account.data[8..] // Skip discriminator
    ).expect("Failed to deserialize collection data");

    assert_eq!(collection_data.name, "Universal NFT Collection");
    assert_eq!(collection_data.symbol, "UNC");
    assert_eq!(collection_data.total_supply, 0);
}

#[tokio::test]
async fn test_mint_nft() {
    let program_test = ProgramTest::new(
        "solana_universal_nft",
        PROGRAM_ID,
        processor!(solana_universal_nft::entry),
    );
    
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Initialize collection first
    let (collection_pda, _) = Pubkey::find_program_address(
        &[b"nft_collection"],
        &PROGRAM_ID,
    );

    // Initialize collection
    let initialize_ix = program_instruction::Initialize {
        name: "Test Collection".to_string(),
        symbol: "TEST".to_string(),
        base_uri: "https://test.com/".to_string(),
    };

    let init_accounts = solana_universal_nft::accounts::Initialize {
        authority: payer.pubkey(),
        nft_collection: collection_pda,
        system_program: solana_program::system_program::id(),
    };

    let init_instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: init_accounts.to_account_metas(None),
        data: initialize_ix.data(),
    };

    let init_transaction = Transaction::new_signed_with_payer(
        &[init_instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    banks_client.process_transaction(init_transaction).await.unwrap();

    // Now mint an NFT
    let token_id = 1u64;
    let (authority_pda, _) = Pubkey::find_program_address(
        &[b"authority"],
        &PROGRAM_ID,
    );

    let (nft_mint_pda, _) = Pubkey::find_program_address(
        &[b"nft_mint", &token_id.to_le_bytes()],
        &PROGRAM_ID,
    );

    let owner_token_account = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
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

    let mint_ix = program_instruction::MintNft {
        token_id,
        name: "Test NFT #1".to_string(),
        uri: "https://test.com/1".to_string(),
        creator_fee: 500, // 5%
    };

    let mint_accounts = solana_universal_nft::accounts::MintNft {
        owner: payer.pubkey(),
        nft_collection: collection_pda,
        authority: authority_pda,
        nft_mint: nft_mint_pda,
        nft_token_account: owner_token_account,
        nft_metadata: nft_metadata_pda,
        token_program: spl_token::id(),
        metadata_program: mpl_token_metadata::ID,
        system_program: solana_program::system_program::id(),
        rent: solana_program::sysvar::rent::id(),
    };

    let mint_instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: mint_accounts.to_account_metas(None),
        data: mint_ix.data(),
    };

    let mint_transaction = Transaction::new_signed_with_payer(
        &[mint_instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(mint_transaction).await;
    assert!(result.is_ok(), "Mint NFT failed: {:?}", result);

    // Verify NFT was minted
    let token_account = banks_client
        .get_account(owner_token_account)
        .await
        .expect("Failed to get token account")
        .expect("Token account not found");

    let token_data = TokenAccount::try_deserialize(
        &mut &token_account.data[..]
    ).expect("Failed to deserialize token account");

    assert_eq!(token_data.amount, 1);
    assert_eq!(token_data.mint, nft_mint_pda);
    assert_eq!(token_data.owner, payer.pubkey());
}

#[tokio::test]
async fn test_cross_chain_nft_transfer() {
    let program_test = ProgramTest::new(
        "solana_universal_nft",
        PROGRAM_ID,
        processor!(solana_universal_nft::entry),
    );
    
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Setup: Initialize collection and mint an NFT
    setup_collection_and_nft(&mut banks_client, &payer, recent_blockhash).await;

    let token_id = 1u64;
    let destination_chain_id = 56u64; // BSC chain ID
    let destination_address = [1u8; 20]; // Mock Ethereum address

    let (nft_mint_pda, _) = Pubkey::find_program_address(
        &[b"nft_mint", &token_id.to_le_bytes()],
        &PROGRAM_ID,
    );

    let (authority_pda, _) = Pubkey::find_program_address(
        &[b"authority"],
        &PROGRAM_ID,
    );

    let owner_token_account = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
        &nft_mint_pda,
    );

    let program_token_account = spl_associated_token_account::get_associated_token_address(
        &authority_pda,
        &nft_mint_pda,
    );

    let (cross_chain_state_pda, _) = Pubkey::find_program_address(
        &[b"cross_chain_state", &token_id.to_le_bytes()],
        &PROGRAM_ID,
    );

    let (nft_metadata_pda, _) = Pubkey::find_program_address(
        &[
            b"metadata",
            mpl_token_metadata::ID.as_ref(),
            nft_mint_pda.as_ref(),
        ],
        &mpl_token_metadata::ID,
    );

    let send_ix = program_instruction::SendNftCrossChain {
        token_id,
        destination_chain_id,
        destination_address,
        revert_options: None,
    };

    let send_accounts = solana_universal_nft::accounts::SendNftCrossChain {
        owner: payer.pubkey(),
        authority: authority_pda,
        nft_token_account: owner_token_account,
        program_nft_account: program_token_account,
        nft_mint: nft_mint_pda,
        nft_metadata: nft_metadata_pda,
        cross_chain_state: cross_chain_state_pda,
        gateway_program: Pubkey::new_unique(), // Mock gateway program
        gateway_state: Pubkey::new_unique(),   // Mock gateway state
        token_program: spl_token::id(),
        metadata_program: mpl_token_metadata::ID,
        system_program: solana_program::system_program::id(),
    };

    let send_instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: send_accounts.to_account_metas(None),
        data: send_ix.data(),
    };

    let send_transaction = Transaction::new_signed_with_payer(
        &[send_instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(send_transaction).await;
    assert!(result.is_ok(), "Cross-chain transfer failed: {:?}", result);

    // Verify cross-chain state
    let cross_chain_account = banks_client
        .get_account(cross_chain_state_pda)
        .await
        .expect("Failed to get cross-chain state")
        .expect("Cross-chain state not found");

    let cross_chain_data = CrossChainState::try_deserialize(
        &mut &cross_chain_account.data[8..]
    ).expect("Failed to deserialize cross-chain state");

    assert_eq!(cross_chain_data.token_id, token_id);
    assert_eq!(cross_chain_data.original_owner, payer.pubkey());
    assert_eq!(cross_chain_data.destination_chain_id, destination_chain_id);
    assert_eq!(cross_chain_data.status, TransferStatus::Pending);
}

#[tokio::test]
async fn test_on_call_receive_nft() {
    let program_test = ProgramTest::new(
        "solana_universal_nft",
        PROGRAM_ID,
        processor!(solana_universal_nft::entry),
    );
    
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Initialize collection
    let (collection_pda, _) = Pubkey::find_program_address(
        &[b"nft_collection"],
        &PROGRAM_ID,
    );

    setup_collection(&mut banks_client, &payer, recent_blockhash).await;

    // Simulate receiving an NFT from another chain
    let recipient = Keypair::new();
    let sender_address = [2u8; 20]; // Mock sender from another chain
    
    let nft_data = NftTransferData {
        token_id: 100,
        name: "Cross-chain NFT".to_string(),
        symbol: "XC".to_string(),
        uri: "https://crosschain.com/100".to_string(),
        creator_fee: 250, // 2.5%
    };

    let data = borsh::to_vec(&nft_data).unwrap();

    let (authority_pda, _) = Pubkey::find_program_address(
        &[b"authority"],
        &PROGRAM_ID,
    );

    let (nft_mint_pda, _) = Pubkey::find_program_address(
        &[b"nft_mint", &0u64.to_le_bytes()], // Using total_supply as token ID
        &PROGRAM_ID,
    );

    let recipient_token_account = spl_associated_token_account::get_associated_token_address(
        &recipient.pubkey(),
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

    let on_call_accounts = solana_universal_nft::accounts::OnCall {
        recipient: recipient.pubkey(),
        nft_collection: collection_pda,
        authority: authority_pda,
        nft_mint: nft_mint_pda,
        recipient_token_account,
        nft_metadata: nft_metadata_pda,
        token_program: spl_token::id(),
        metadata_program: mpl_token_metadata::ID,
        system_program: solana_program::system_program::id(),
        rent: solana_program::sysvar::rent::id(),
    };

    let on_call_ix = program_instruction::OnCall {
        amount: 0, // No SOL transfer for NFT
        sender: sender_address,
        data,
    };

    let on_call_instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: on_call_accounts.to_account_metas(None),
        data: on_call_ix.data(),
    };

    // Fund recipient account
    let fund_ix = system_instruction::transfer(
        &payer.pubkey(),
        &recipient.pubkey(),
        1_000_000_000, // 1 SOL
    );

    let transaction = Transaction::new_signed_with_payer(
        &[fund_ix, on_call_instruction],
        Some(&payer.pubkey()),
        &[&payer, &recipient],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "OnCall receive NFT failed: {:?}", result);

    // Verify NFT was received
    let token_account = banks_client
        .get_account(recipient_token_account)
        .await
        .expect("Failed to get recipient token account")
        .expect("Recipient token account not found");

    let token_data = TokenAccount::try_deserialize(
        &mut &token_account.data[..]
    ).expect("Failed to deserialize recipient token account");

    assert_eq!(token_data.amount, 1);
    assert_eq!(token_data.owner, recipient.pubkey());
}

// Helper functions
async fn setup_collection(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: solana_sdk::hash::Hash,
) {
    let (collection_pda, _) = Pubkey::find_program_address(
        &[b"nft_collection"],
        &PROGRAM_ID,
    );

    let initialize_ix = program_instruction::Initialize {
        name: "Test Collection".to_string(),
        symbol: "TEST".to_string(),
        base_uri: "https://test.com/".to_string(),
    };

    let accounts = solana_universal_nft::accounts::Initialize {
        authority: payer.pubkey(),
        nft_collection: collection_pda,
        system_program: solana_program::system_program::id(),
    };

    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: accounts.to_account_metas(None),
        data: initialize_ix.data(),
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );

    banks_client.process_transaction(transaction).await.unwrap();
}

async fn setup_collection_and_nft(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: solana_sdk::hash::Hash,
) {
    setup_collection(banks_client, payer, recent_blockhash).await;

    // Mint an NFT
    let token_id = 1u64;
    let (collection_pda, _) = Pubkey::find_program_address(
        &[b"nft_collection"],
        &PROGRAM_ID,
    );

    let (authority_pda, _) = Pubkey::find_program_address(
        &[b"authority"],
        &PROGRAM_ID,
    );

    let (nft_mint_pda, _) = Pubkey::find_program_address(
        &[b"nft_mint", &token_id.to_le_bytes()],
        &PROGRAM_ID,
    );

    let owner_token_account = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
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

    let mint_ix = program_instruction::MintNft {
        token_id,
        name: "Test NFT #1".to_string(),
        uri: "https://test.com/1".to_string(),
        creator_fee: 500,
    };

    let mint_accounts = solana_universal_nft::accounts::MintNft {
        owner: payer.pubkey(),
        nft_collection: collection_pda,
        authority: authority_pda,
        nft_mint: nft_mint_pda,
        nft_token_account: owner_token_account,
        nft_metadata: nft_metadata_pda,
        token_program: spl_token::id(),
        metadata_program: mpl_token_metadata::ID,
        system_program: solana_program::system_program::id(),
        rent: solana_program::sysvar::rent::id(),
    };

    let mint_instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: mint_accounts.to_account_metas(None),
        data: mint_ix.data(),
    };

    let mint_transaction = Transaction::new_signed_with_payer(
        &[mint_instruction],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );

    banks_client.process_transaction(mint_transaction).await.unwrap();
}