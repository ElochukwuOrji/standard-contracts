# Solana Universal NFT Program

A comprehensive Solana program for Universal NFTs with seamless cross-chain interoperability via ZetaChain. This program enables NFT minting, transfers, and cross-chain interactions between Solana and other blockchain networks.

## 🌟 Features

- **Universal NFT Support**: Full-featured NFT implementation with metadata and royalties
- **Cross-Chain Interoperability**: Send and receive NFTs across multiple blockchains via ZetaChain
- **Security-First Design**: Built with TSS signatures, replay protection, and secure state management
- **Solana Optimized**: Efficient compute budget usage, proper rent exemption, and account management
- **Developer-Friendly**: Comprehensive SDK, detailed documentation, and example usage

## 🏗️ Architecture

### Core Components

1. **Universal NFT Program** (`programs/solana-universal-nft/src/lib.rs`)
   - Main Solana program handling NFT operations
   - Cross-chain message encoding/decoding
   - Integration with ZetaChain Gateway

2. **Client SDK** (`sdk/src/lib.rs`)
   - High-level TypeScript/Rust interface
   - Transaction builders and helpers
   - Cross-chain utilities

3. **Test Suite** (`tests/`)
   - Comprehensive integration tests
   - Cross-chain simulation
   - Security and edge case testing

### Key Features

#### NFT Operations
- `initialize`: Create a new NFT collection
- `mint_nft`: Mint NFTs locally on Solana
- `send_nft_cross_chain`: Send NFTs to other chains via ZetaChain
- `on_call`: Receive incoming NFTs from other chains
- `on_revert`: Handle failed cross-chain transfers
- `update_nft_metadata`: Update NFT metadata (authority only)

#### Cross-Chain Integration
- **ZetaChain Gateway Integration**: Direct integration with ZetaChain's Solana gateway
- **Multi-Chain Support**: Ethereum, BSC, Polygon, Arbitrum, Optimism, and more
- **Revert Protection**: Comprehensive error handling with revert options
- **State Tracking**: Track cross-chain transfer status and history

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+
- Solana CLI 1.17+
- Anchor Framework 0.29+
- Node.js 16+ (for JavaScript SDK)

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/solana-universal-nft
cd solana-universal-nft

# Install dependencies
cargo build

# Run tests
cargo test
anchor test
```

### Basic Usage

#### 1. Initialize a Collection

```rust
use solana_universal_nft_sdk::UniversalNftClient;
use anchor_client::Cluster;
use std::rc::Rc;

// Initialize client
let payer = Rc::new(keypair_from_file("~/.config/solana/id.json")?);
let client = UniversalNftClient::new(Cluster::Devnet, payer, None);

// Create collection
let signature = client.initialize_collection(
    "My Universal Collection".to_string(),
    "MUC".to_string(),
    "https://api.mycollection.com/metadata/".to_string(),
).await?;
```

#### 2. Mint an NFT

```rust
let signature = client.mint_nft(
    1, // token_id
    "My First NFT".to_string(),
    "https://api.mycollection.com/metadata/1".to_string(),
    500, // 5% creator fee
    None, // mint to payer
).await?;
```

#### 3. Send NFT Cross-Chain

```rust
use solana_universal_nft_sdk::{chain_ids, utils};

// Send to Ethereum
let eth_address = utils::parse_eth_address("0x742d35Cc6635Cb8532")?;
let signature = client.send_nft_cross_chain(
    1, // token_id
    chain_ids::ETHEREUM,
    eth_address,
    None, // from payer
    None, // no revert options
).await?;
```

#### 4. Check Transfer Status

```rust
let status = client.get_cross_chain_status(1).await?;
println!("Transfer status: {:?}", status.status);
```

## 📋 Program Instructions

### Core Instructions

#### `initialize`
Initialize a new NFT collection with metadata.

**Parameters:**
- `name`: Collection name (max 32 chars)
- `symbol`: Collection symbol (max 10 chars)  
- `base_uri`: Base URI for metadata (max 200 chars)

#### `mint_nft`
Mint a new NFT with unique token ID.

**Parameters:**
- `token_id`: Unique identifier for the NFT
- `name`: NFT name
- `uri`: Metadata URI
- `creator_fee`: Creator royalty in basis points (500 = 5%)

#### `send_nft_cross_chain`
Initiate cross-chain NFT transfer via ZetaChain.

**Parameters:**
- `token_id`: NFT to transfer
- `destination_chain_id`: Target blockchain ID
- `destination_address`: Recipient address (20 bytes)
- `revert_options`: Optional revert configuration

#### `on_call`
Receive incoming NFT from another blockchain.

**Parameters:**
- `amount`: Associated token amount (usually 0 for NFTs)
- `sender`: Origin address from source chain
- `data`: Encoded NFT metadata

#### `on_revert`
Handle failed cross-chain transfer and return NFT.

**Parameters:**
- `amount`: Original transfer amount
- `sender`: Original sender address
- `data`: Revert-specific data

## 🔐 Security Features

### TSS Integration
- **Threshold Signature Schemes**: Secure multi-party signatures for cross-chain operations
- **Replay Protection**: Nonce-based transaction uniqueness
- **Authority Validation**: Strict signer verification

### Account Security
- **PDA-Based Architecture**: All critical accounts use Program Derived Addresses
- **Rent Exemption**: Proper account sizing and rent handling
- **Access Control**: Role-based permissions for sensitive operations

### Cross-Chain Safety
- **Revert Options**: Comprehensive failure handling
- **State Tracking**: Monitor transfer status and prevent double-spending
- **Timeout Protection**: Handle stuck transactions gracefully

## 🌐 Supported Chains

| Chain      | Chain ID | Status       |
|------------|----------|--------------|
| Ethereum   | 1        | ✅ Supported |
| BSC        | 56       | ✅ Supported |
| Polygon    | 137      | ✅ Supported |
| Arbitrum   | 42161    | ✅ Supported |
| Optimism   | 10       | ✅ Supported |
| Avalanche  | 43114    | ✅ Supported |
| ZetaChain  | 7000     | ✅ Native    |

## 📊 Account Structure

### NFT Collection Account
```rust
pub struct NftCollection {
    pub authority: Pubkey,      // Collection authority
    pub name: String,           // Collection name
    pub symbol: String,         // Collection symbol  
    pub base_uri: String,       // Base metadata URI
    pub total_supply: u64,      // Total NFTs minted
    pub bump: u8,               // PDA bump seed
}
```

### Cross-Chain State Account
```rust
pub struct CrossChainState {
    pub token_id: u64,               // NFT token ID
    pub original_owner: Pubkey,      // Original owner
    pub destination_chain_id: u64,   // Target chain
    pub destination_address: [u8; 20], // Target address
    pub status: TransferStatus,      // Transfer status
    pub bump: u8,                    // PDA bump seed
}
```

## 🧪 Testing

### Unit Tests
```bash
cargo test
```

### Integration Tests
```bash
anchor test
```

### Cross-Chain Simulation
```bash
# Start local validator
solana-test-validator

# Run cross-chain tests
anchor test --skip-local-validator
```

## 📚 Examples

### Complete NFT Lifecycle

```rust
use solana_universal_nft_sdk::*;

#[tokio::main]
async fn main() -> Result<()> {
    let client = UniversalNftClient::new(Cluster::Devnet, payer, None);
    
    // 1. Initialize collection
    client.initialize_collection(
        "GameFi Collection".to_string(),
        "GAME".to_string(),
        "https://gamefi.example.com/".to_string(),
    ).await?;
    
    // 2. Mint NFT
    client.mint_nft(
        1,
        "Legendary Sword".to_string(),
        "https://gamefi.example.com/1".to_string(),
        1000, // 10% royalty
        None,
    ).await?;
    
    // 3. Send to Ethereum
    let eth_recipient = utils::parse_eth_address("0x742d35Cc6635Cb8532")?;
    client.send_nft_cross_chain(
        1,
        chain_ids::ETHEREUM,
        eth_recipient,
        None,
        Some(RevertOptions {
            revert_address: client.program.payer(),
            abort_address: utils::pubkey_to_eth_address(&client.program.payer()),
            call_on_revert: true,
            revert_message: b"Game item transfer".to_vec(),
            on_revert_gas_limit: 100_000,
        }),
    ).await?;
    
    // 4. Monitor status
    loop {
        let status = client.get_cross_chain_status(1).await?;
        match status.status {
            TransferStatus::Completed => {
                println!("Transfer completed successfully!");
                break;
            },
            TransferStatus::Reverted => {
                println!("Transfer was reverted");
                break;
            },
            TransferStatus::Pending => {
                println!("Transfer still pending...");
                tokio::time::sleep(Duration::from_secs(5)).await;
            },
        }
    }
    
    Ok(())
}
```

### Error Handling

```rust
match client.send_nft_cross_chain(token_id, chain_id, address, None, None).await {
    Ok(signature) => println!("Transfer initiated: {}", signature),
    Err(UniversalNftError::InvalidTokenId) => println!("Invalid token ID"),
    Err(UniversalNftError::Unauthorized) => println!("Not authorized"),
    Err(UniversalNftError::CrossChainTransferFailed) => println!("Cross-chain transfer failed"),
    Err(e) => println!("Other error: {}", e),
}
```

## 🔧 Configuration

### Program Configuration
```toml
# Anchor.toml
[programs.localnet]
solana_universal_nft = "D9Wnf46z72Wq6g7s4X7KZ8u6y6LmpDrDYKaLi6jHKdZr"

[programs.devnet]
solana_universal_nft = "D9Wnf46z72Wq6g7s4X7KZ8u6y6LmpDrDYKaLi6jHKdZr"

[registry]
url = "https://api.apr.dev"

[provider]
cluster = "devnet"
wallet = "~/.config/solana/id.json"
```

### Environment Variables
```bash
# Set Solana cluster
export SOLANA_CLUSTER=devnet

# Set ZetaChain gateway 
export ZETACHAIN_GATEWAY=ZETAjseVjuFsxdRxo6MmTCvqFwb3ZHUx56Co3vCmGis

# Set RPC endpoint
export SOLANA_RPC_URL=https://api.devnet.solana.com
```

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

### Code Standards
- Follow Rust best practices
- Add comprehensive tests
- Document public APIs
- Use conventional commits

## 📄 License

This project is licensed under the MIT License.

## 🙏 Acknowledgments

- [ZetaChain](https://zetachain.com) for cross-chain infrastructure
- [Anchor Framework](https://anchor-lang.com) for Solana development
- [Metaplex](https://metaplex.com) for NFT standards
- Solana Foundation for the blockchain platform

## 📞 Support

- [Discord](https://discord.gg/zetachain) - Join the ZetaChain community
- [GitHub Issues](https://github.com/your-org/solana-universal-nft/issues) - Report bugs
- [Documentation](https://docs.zetachain.com) - ZetaChain docs
- [Twitter](https://twitter.com/zetachain) - Follow updates

---

**Built for the ZetaChain Universal NFT Bounty** 🏆
