use anchor_client::{solana_sdk::signature::read_keypair_file, Client, Cluster};
use solana_universal_nft_sdk::*;
use std::{rc::Rc, time::Duration};
use tokio::time::sleep;

/// Complete demonstration of cross-chain NFT functionality
/// This example shows the full lifecycle of a Universal NFT:
/// 1. Initialize collection on Solana
/// 2. Mint NFT on Solana  
/// 3. Transfer NFT to Ethereum via ZetaChain
/// 4. Monitor transfer status
/// 5. Handle potential reverts

#[tokio::main]
async fn main() -> Result<()> {
    println!("🌟 Solana Universal NFT Cross-Chain Demo");
    println!("═══════════════════════════════════════════════════════════════");

    // Setup client
    let payer = Rc::new(
        read_keypair_file(&*shellexpand::tilde("~/.config/solana/id.json"))
            .expect("Failed to read keypair file"),
    );
    
    let client = UniversalNftClient::new(Cluster::Devnet, payer.clone(), None);
    
    println!("📊 Wallet: {}", payer.pubkey());
    println!("🌐 Cluster: Devnet");
    println!("📦 Program ID: {}", solana_universal_nft::ID);
    println!();

    // Step 1: Initialize Collection
    println!("🔧 Step 1: Initializing Universal NFT Collection...");
    
    let collection_info = match client.get_collection().await {
        Ok(collection) => {
            println!("✅ Collection already exists: {}", collection.name);
            collection
        }
        Err(_) => {
            println!("📝 Creating new collection...");
            let signature = client
                .initialize_collection(
                    "ZetaChain Gaming NFTs".to_string(),
                    "ZGN".to_string(),
                    "https://api.zetagaming.example.com/metadata/".to_string(),
                )
                .await?;

            println!("✅ Collection initialized! Signature: {}", signature);
            
            // Wait for confirmation and fetch collection
            sleep(Duration::from_secs(2)).await;
            client.get_collection().await?
        }
    };

    println!("📊 Collection Details:");
    println!("   Name: {}", collection_info.name);
    println!("   Symbol: {}", collection_info.symbol);
    println!("   Total Supply: {}", collection_info.total_supply);
    println!();

    // Step 2: Mint a Gaming NFT
    println!("🎮 Step 2: Minting a Gaming NFT...");
    
    let token_id = collection_info.total_supply + 1;
    let nft_name = format!("Legendary Sword #{}", token_id);
    let nft_uri = format!("https://api.zetagaming.example.com/metadata/{}", token_id);
    
    println!("🗡️  Minting: {}", nft_name);
    println!("🔗 Metadata: {}", nft_uri);
    
    let mint_signature = client
        .mint_nft(
            token_id,
            nft_name.clone(),
            nft_uri.clone(),
            750, // 7.5% creator royalty
            None, // mint to payer
        )
        .await?;

    println!("✅ NFT minted! Signature: {}", mint_signature);
    
    // Verify NFT was minted
    let nft_mint = client.get_nft_mint_address(token_id);
    let token_account = client.get_token_account_address(token_id, payer.pubkey());
    
    println!("📦 NFT Mint: {}", nft_mint);
    println!("🏦 Token Account: {}", token_account);
    println!();

    // Step 3: Prepare Cross-Chain Transfer
    println!("🌉 Step 3: Preparing Cross-Chain Transfer to Ethereum...");
    
    // Example Ethereum recipient address (replace with actual address)
    let eth_recipient = utils::parse_eth_address("0x742d35Cc6635Cb85324D6cF6D9AFA26dEE26D45f")
        .expect("Invalid Ethereum address");
    
    println!("🎯 Destination: Ethereum (Chain ID: {})", chain_ids::ETHEREUM);
    println!("📨 Recipient: {}", utils::eth_address_to_string(&eth_recipient));
    
    // Configure revert options for safety
    let revert_options = RevertOptions {
        revert_address: payer.pubkey(),
        abort_address: utils::pubkey_to_eth_address(&payer.pubkey()),
        call_on_revert: true,
        revert_message: format!("Cross-chain transfer of {}", nft_name).into_bytes(),
        on_revert_gas_limit: 100_000,
    };
    
    println!("🛡️  Revert protection enabled");
    println!("   Revert Address: {}", revert_options.revert_address);
    println!("   Gas Limit: {}", revert_options.on_revert_gas_limit);
    println!();

    // Step 4: Execute Cross-Chain Transfer
    println!("🚀 Step 4: Executing Cross-Chain Transfer...");
    
    let transfer_signature = client
        .send_nft_cross_chain(
            token_id,
            chain_ids::ETHEREUM,
            eth_recipient,
            None, // from payer
            Some(revert_options),
        )
        .await?;

    println!("✅ Cross-chain transfer initiated!");
    println!("📝 Transaction: {}", transfer_signature);
    println!();

    // Step 5: Monitor Transfer Status
    println!("👁️  Step 5: Monitoring Transfer Status...");
    println!("⏳ Waiting for cross-chain confirmation...");
    
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(300); // 5 minute timeout
    
    loop {
        if start_time.elapsed() > timeout {
            println!("⏰ Timeout reached. Transfer may still be processing...");
            break;
        }

        match client.get_cross_chain_status(token_id).await {
            Ok(status) => {
                let elapsed = start_time.elapsed();
                println!("📊 Status Update ({:.1}s elapsed):", elapsed.as_secs_f32());
                println!("   Token ID: {}", status.token_id);
                println!("   Original Owner: {}", status.original_owner);
                println!("   Destination Chain: {}", status.destination_chain_id);
                println!("   Status: {:?}", status.status);
                
                match status.status {
                    TransferStatus::Completed => {
                        println!("🎉 Transfer completed successfully!");
                        println!("✅ NFT is now available on Ethereum");
                        break;
                    }
                    TransferStatus::Reverted => {
                        println!("🔄 Transfer was reverted - NFT returned to original owner");
                        println!("💡 Check revert reason and try again if needed");
                        break;
                    }
                    TransferStatus::Pending => {
                        println!("⏳ Still pending... waiting 10 seconds");
                        sleep(Duration::from_secs(10)).await;
                    }
                }
            }
            Err(e) => {
                println!("❌ Error checking status: {}", e);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }

    println!();

    // Step 6: Demonstrate Additional Features
    println!("🔧 Step 6: Additional Features Demo...");
    
    // Get updated collection info
    let updated_collection = client.get_collection().await?;
    println!("📊 Updated Collection Total Supply: {}", updated_collection.total_supply);
    
    // Example: Update metadata (if you're the authority)
    println!("📝 Metadata update example:");
    println!("   Current URI: {}", nft_uri);
    
    let new_uri = format!("https://api.zetagaming.example.com/metadata/{}/v2", token_id);
    println!("   New URI: {}", new_uri);
    
    match client
        .update_nft_metadata(
            token_id,
            Some(new_uri.clone()),
            Some(format!("{} (Enhanced)", nft_name)),
        )
        .await
    {
        Ok(sig) => println!("✅ Metadata updated! Signature: {}", sig),
        Err(e) => println!("⚠️  Metadata update failed (may not be authority): {}", e),
    }
    
    println!();

    // Step 7: Cross-Chain Interaction Examples
    println!("🌐 Step 7: Cross-Chain Integration Examples...");
    
    // Example of different chain targets
    let chain_examples = vec![
        (chain_ids::BSC, "Binance Smart Chain"),
        (chain_ids::POLYGON, "Polygon"),
        (chain_ids::ARBITRUM, "Arbitrum"),
        (chain_ids::OPTIMISM, "Optimism"),
        (chain_ids::AVALANCHE, "Avalanche"),
    ];
    
    println!("🎯 Supported destination chains:");
    for (chain_id, name) in chain_examples {
        println!("   {} (ID: {})", name, chain_id);
    }
    
    // Example of batch operations
    println!();
    println!("📦 Batch Operation Example:");
    println!("   For minting multiple NFTs, you can:");
    println!("   1. Use a loop with different token IDs");
    println!("   2. Implement batch minting in your application");
    println!("   3. Consider compute budget limits for large batches");
    
    // Gaming-specific use cases
    println!();
    println!("🎮 Gaming Use Cases:");
    println!("   1. Cross-chain item trading");
    println!("   2. Multi-chain game asset portability");
    println!("   3. Interoperable character progression");
    println!("   4. Cross-platform tournament rewards");
    
    // DeFi integration examples
    println!();
    println!("💰 DeFi Integration Possibilities:");
    println!("   1. Cross-chain NFT lending/borrowing");
    println!("   2. Multi-chain liquidity pools");
    println!("   3. Cross-chain NFT fractionalization");
    println!("   4. Universal yield farming with NFT rewards");

    println!();
    println!("🎊 Demo completed successfully!");
    println!("═══════════════════════════════════════════════════════════════");
    
    // Final summary
    println!("📋 Summary:");
    println!("✅ Collection initialized: {}", collection_info.name);
    println!("✅ NFT minted: {} (Token ID: {})", nft_name, token_id);
    println!("✅ Cross-chain transfer initiated to Ethereum");
    println!("✅ Demonstrated monitoring and error handling");
    println!("✅ Showcased additional features and use cases");
    println!();
    println!("🔗 Useful Resources:");
    println!("   📖 Documentation: https://docs.zetachain.com");
    println!("   💬 Discord: https://discord.gg/zetachain");
    println!("   🐙 GitHub: https://github.com/zeta-chain");
    println!("   🐦 Twitter: https://twitter.com/zetachain");

    Ok(())
}

/// Helper function to demonstrate error handling patterns
async fn demonstrate_error_handling(client: &UniversalNftClient) {
    println!("🛡️  Error Handling Examples:");
    
    // Example 1: Invalid token ID
    match client.get_cross_chain_status(99999).await {
        Ok(_) => println!("   Unexpected success for invalid token ID"),
        Err(UniversalNftError::NftNotFound) => {
            println!("   ✅ Correctly handled NFT not found error");
        }
        Err(e) => println!("   ⚠️  Other error for invalid token ID: {}", e),
    }
    
    // Example 2: Invalid Ethereum address
    match utils::parse_eth_address("invalid_address") {
        Ok(_) => println!("   Unexpected success for invalid address"),
        Err(_) => println!("   ✅ Correctly handled invalid Ethereum address"),
    }
    
    // Example 3: Network timeouts
    println!("   💡 Network timeouts should be handled with retries");
    println!("   💡 Use exponential backoff for robust applications");
    println!("   💡 Implement circuit breakers for production systems");
}

/// Example of batch minting for gaming applications
async fn demonstrate_batch_minting(
    client: &UniversalNftClient,
    start_token_id: u64,
    count: u32,
) -> Result<Vec<u64>> {
    println!("📦 Batch Minting {} NFTs starting from token ID {}...", count, start_token_id);
    
    let mut minted_tokens = Vec::new();
    
    for i in 0..count {
        let token_id = start_token_id + i as u64;
        let name = format!("Gaming Item #{}", token_id);
        let uri = format!("https://api.game.com/items/{}", token_id);
        
        match client
            .mint_nft(token_id, name.clone(), uri, 500, None)
            .await
        {
            Ok(signature) => {
                println!("   ✅ Minted {}: {}", name, signature);
                minted_tokens.push(token_id);
            }
            Err(e) => {
                println!("   ❌ Failed to mint {}: {}", name, e);
                break; // Stop on first error
            }
        }
        
        // Small delay to avoid rate limits
        sleep(Duration::from_millis(100)).await;
    }
    
    println!("📊 Batch minting completed: {}/{} successful", minted_tokens.len(), count);
    Ok(minted_tokens)
}

/// Utility function to format duration nicely
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}