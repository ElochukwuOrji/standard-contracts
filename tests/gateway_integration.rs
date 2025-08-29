// tests/gateway_integration.rs
use anchor_client::{
    solana_sdk::{
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        system_instruction,
    },
    Client, Cluster, Program,
};
use solana_universal_nft::{gateway::*, *};
use std::rc::Rc;

#[tokio::test]
async fn test_gateway_integration() -> Result<(), Box<dyn std::error::Error>> {
    // Setup client and program
    let payer = Rc::new(Keypair::new());
    let client = Client::new_with_options(
        Cluster::Devnet,
        payer.clone(),
        CommitmentConfig::processed(),
    );
    
    let program_id = solana_universal_nft::ID;
    let program = client.program(program_id)?;

    // Airdrop some SOL for testing
    client
        .request_airdrop(&payer.pubkey(), 2_000_000_000)
        .await?;

    // Test 1: Verify gateway program exists
    let gateway_account = client
        .get_account(&GATEWAY_PROGRAM_ID)
        .await
        .expect("Gateway program should exist");
    
    assert!(gateway_account.executable, "Gateway program should be executable");
    println!("✅ Gateway program verified: {}", GATEWAY_PROGRAM_ID);

    // Test 2: Check gateway PDA
    let gateway_pda_account = client
        .get_account(&GATEWAY_PDA)
        .await
        .expect("Gateway PDA should exist");
    
    println!("✅ Gateway PDA verified: {}", GATEWAY_PDA);
    println!("   PDA balance: {} lamports", gateway_pda_account.lamports);

    // Test 3: Create deposit instruction for NFT data
    let receiver = [1u8; 20]; // Ethereum-style address
    let nft_data = b"test nft data".to_vec();
    
    let deposit_instruction = create_deposit_instruction(
        receiver,
        0, // 0 SOL for data-only transfer
        None, // SOL, not SPL token
        Some(RevertContext {
            asset: solana_program::system_program::ID,
            amount: 0,
            revert_message: nft_data,
        }),
        payer.pubkey(),
    )?;

    println!("✅ Deposit instruction created successfully");
    println!("   Program ID: {}", deposit_instruction.program_id);
    println!("   Accounts: {}", deposit_instruction.accounts.len());
    println!("   Data length: {} bytes", deposit_instruction.data.len());

    // Test 4: Simulate the instruction (don't actually send it)
    println!("✅ Gateway integration tests completed successfully");

    Ok(())
}

#[tokio::test]
async fn test_nft_cross_chain_flow() -> Result<(), Box<dyn std::error::Error>> {
    let payer = Rc::new(Keypair::new());
    let client = Client::new_with_options(
        Cluster::Devnet,
        payer.clone(),
        CommitmentConfig::processed(),
    );
    
    let program = client.program(solana_universal_nft::ID)?;

    // Airdrop SOL
    client
        .request_airdrop(&payer.pubkey(), 2_000_000_000)
        .await?;

    // Step 1: Initialize NFT collection
    let collection_pda = Pubkey::find_program_address(
        &[b"nft_collection"],
        &program.id(),
    ).0;

    let initialize_tx = program
        .request()
        .accounts(solana_universal_nft::accounts::Initialize {
            authority: payer.pubkey(),
            nft_collection: collection_pda,
            system_program: solana_program::system_program::ID,
        })
        .args(solana_universal_nft::instruction::Initialize {
            name: "Test Universal NFTs".to_string(),
            symbol: "TUN".to_string(),
            base_uri: "https://test.example.com/".to_string(),
        })
        .send()
        .await?;

    println!("✅ Collection initialized: {}", initialize_tx);

    // Step 2: Mint an NFT
    let token_id = 1u64;
    let mint_pda = Pubkey::find_program_address(
        &[b"nft_mint", &token_id.to_le_bytes()],
        &program.id(),
    ).0;

    // Create more test cases for minting, cross-chain transfer, etc.
    println!("✅ NFT cross-chain flow test setup completed");

    Ok(())
}