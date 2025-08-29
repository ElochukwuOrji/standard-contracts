# Security Guide for Solana Universal NFT Program

This document outlines the security measures, best practices, and considerations implemented in the Solana Universal NFT program for ZetaChain cross-chain interoperability.

## 🛡️ Security Features

### 1. Program Derived Addresses (PDAs)

All critical accounts use PDAs to ensure deterministic and secure address generation:

```rust
// Collection PDA
let (collection_pda, bump) = Pubkey::find_program_address(
    &[b"nft_collection"],
    &program_id,
);

// Authority PDA
let (authority_pda, bump) = Pubkey::find_program_address(
    &[b"authority"],
    &program_id,
);

// NFT Mint PDA
let (nft_mint_pda, bump) = Pubkey::find_program_address(
    &[b"nft_mint", &token_id.to_le_bytes()],
    &program_id,
);
```

**Security Benefits:**
- No private key management for program accounts
- Deterministic address generation
- Program-controlled access
- Impossible to create conflicting accounts

### 2. Access Control and Authorization

#### Signer Validation
```rust
#[account(mut)]
pub owner: Signer<'info>,
```

#### Authority Checks
```rust
require!(
    ctx.accounts.nft_token_account.amount == 1,
    ErrorCode::NotOwner
);

require!(
    cross_chain_state.original_owner == sender,
    ErrorCode::UnauthorizedRevert
);
```

#### Role-Based Permissions
- **Collection Authority**: Can update collection metadata, mint NFTs
- **NFT Owner**: Can transfer and send cross-chain
- **Program Authority**: Internal operations only

### 3. Cross-Chain Security

#### TSS Integration
```rust
// TSS signature verification in ZetaChain Gateway
pub fn verify_tss_signature(
    message: &[u8],
    signature: &[u8],
    tss_address: &Pubkey,
) -> Result<bool> {
    // Implementation details handled by ZetaChain Gateway
}
```

#### Replay Protection
```rust
pub struct CrossChainState {
    pub token_id: u64,
    pub original_owner: Pubkey,
    pub destination_chain_id: u64,
    pub destination_address: [u8; 20],
    pub status: TransferStatus,
    pub nonce: u64, // Prevents replay attacks
    pub timestamp: i64, // Enables timeout handling
}
```

#### Revert Protection
```rust
pub struct RevertOptions {
    pub revert_address: Pubkey,        // Fallback address on Solana
    pub abort_address: [u8; 20],       // Fallback address on ZetaChain
    pub call_on_revert: bool,          // Enable revert callbacks
    pub revert_message: Vec<u8>,       // Custom revert data
    pub on_revert_gas_limit: u64,      // Gas for revert execution
}
```

### 4. State Management Security

#### Account Validation
```rust
#[account(
    mut,
    seeds = [CROSS_CHAIN_STATE_SEED, &token_id.to_le_bytes()],
    bump = cross_chain_state.bump
)]
pub cross_chain_state: Account<'info, CrossChainState>,
```

#### Data Integrity
```rust
#[account]
#[derive(InitSpace)]
pub struct NftCollection {
    pub authority: Pubkey,
    #[max_len(32)]
    pub name: String,
    #[max_len(10)]
    pub symbol: String,
    #[max_len(200)]
    pub base_uri: String,
    pub total_supply: u64,
    pub bump: u8,
}
```

### 5. Economic Security

#### Rent Exemption
All accounts are created with sufficient lamports for rent exemption:

```rust
#[account(
    init,
    payer = owner,
    space = 8 + NftCollection::INIT_SPACE,
    seeds = [NFT_COLLECTION_SEED],
    bump
)]
pub nft_collection: Account<'info, NftCollection>,
```

#### Fee Management
```rust
// Gateway fees handled by ZetaChain protocol
const CROSS_CHAIN_FEE: u64 = 2_000_000; // 0.002 SOL
```

## 🔒 Security Best Practices

### 1. Input Validation

#### Token ID Validation
```rust
require!(token_id > 0, ErrorCode::InvalidTokenId);
require!(token_id > collection.total_supply, ErrorCode::InvalidTokenId);
```

#### Address Validation
```rust
require!(!destination_address.is_empty(), ErrorCode::EmptyReceiver);
```

#### Data Size Limits
```rust
require!(message.len() <= MAX_MESSAGE_SIZE, ErrorCode::MessageTooLarge);
require!(metadata.name.len() <= 32, ErrorCode::NameTooLong);
```

### 2. Error Handling

#### Custom Error Types
```rust
#[error_code]
pub enum ErrorCode {
    #[msg("Invalid token ID")]
    InvalidTokenId,
    #[msg("Not the owner of the NFT")]
    NotOwner,
    #[msg("Unauthorized revert attempt")]
    UnauthorizedRevert,
    #[msg("Invalid revert state")]
    InvalidRevertState,
    #[msg("Insufficient compute budget")]
    InsufficientComputeBudget,
    #[msg("Cross-chain transfer failed")]
    CrossChainTransferFailed,
}
```

#### Graceful Failures
```rust
pub fn handle_cross_chain_error(&self, error: &CrossChainError) -> Result<()> {
    match error {
        CrossChainError::InsufficientGas => {
            // Log error and revert transaction
            msg!("Insufficient gas for cross-chain operation");
            Err(ErrorCode::CrossChainTransferFailed.into())
        }
        CrossChainError::NetworkTimeout => {
            // Implement retry logic
            msg!("Network timeout, operation may retry");
            Ok(())
        }
        _ => Err(ErrorCode::CrossChainTransferFailed.into()),
    }
}
```

### 3. Compute Budget Management

#### Efficient Account Access
```rust
// Pre-calculate PDAs to avoid repeated derivations
let (mint_pda, mint_bump) = Pubkey::find_program_address(
    &[b"nft_mint", &token_id.to_le_bytes()],
    &program_id,
);
```

#### Optimized Data Structures
```rust
// Use fixed-size arrays where possible
pub destination_address: [u8; 20], // Instead of Vec<u8>

// Pack data efficiently
#[repr(C)]
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct PackedNftData {
    pub token_id: u64,
    pub creator_fee: u16,
    // ... other fields
}
```

### 4. Cross-Chain Message Security

#### Message Encoding
```rust
pub fn encode_nft_transfer_message(
    nft_data: &NftTransferData,
) -> Result<Vec<u8>> {
    // Use deterministic encoding
    borsh::to_vec(nft_data)
        .map_err(|_| ErrorCode::SerializationError.into())
}
```

#### Message Validation
```rust
pub fn validate_cross_chain_message(
    data: &[u8],
    expected_sender: &[u8; 20],
    nonce: u64,
) -> Result<bool> {
    // Verify message format
    let decoded: NftTransferData = borsh::from_slice(data)?;
    
    // Validate sender
    require!(sender == expected_sender, ErrorCode::InvalidSender);
    
    // Check nonce for replay protection
    require!(nonce > last_processed_nonce, ErrorCode::InvalidNonce);
    
    Ok(true)
}
```

## 🚨 Security Audit Checklist

### Program-Level Security

- [ ] **PDA Usage**: All critical accounts use PDAs
- [ ] **Access Control**: Proper signer validation
- [ ] **Input Validation**: All inputs are validated
- [ ] **Error Handling**: Comprehensive error codes
- [ ] **State Management**: Consistent state transitions
- [ ] **Compute Limits**: Efficient resource usage
- [ ] **Rent Exemption**: All accounts properly funded

### Cross-Chain Security

- [ ] **TSS Verification**: ZetaChain TSS signatures validated
- [ ] **Replay Protection**: Nonce-based uniqueness
- [ ] **Message Integrity**: Proper encoding/decoding
- [ ] **Revert Handling**: Safe failure recovery
- [ ] **Timeout Management**: Handle stuck transactions
- [ ] **Fee Validation**: Proper fee payment
- [ ] **Address Validation**: Correct address formats

### Economic Security

- [ ] **Fee Structure**: Reasonable and sustainable fees
- [ ] **Rent Management**: Proper account funding
- [ ] **Token Economics**: Sound tokenomics design
- [ ] **MEV Protection**: Minimize extractable value
- [ ] **Flash Loan Protection**: Prevent manipulation
- [ ] **Oracle Security**: If using price feeds

### Operational Security

- [ ] **Key Management**: Secure authority keys
- [ ] **Upgrade Path**: Safe upgrade mechanism
- [ ] **Monitoring**: Transaction monitoring
- [ ] **Incident Response**: Emergency procedures
- [ ] **Documentation**: Complete security docs
- [ ] **Testing**: Comprehensive test coverage

## 🔧 Security Testing

### Unit Tests
```rust
#[test]
fn test_unauthorized_transfer() {
    // Test that non-owners cannot transfer NFTs
    let unauthorized_user = Keypair::new();
    let result = program.send_nft_cross_chain(
        &unauthorized_user,
        token_id,
        destination_chain,
        destination_address,
    );
    assert!(result.is_err());
}

#[test]
fn test_replay_protection() {
    // Test that duplicate nonces are rejected
    let nonce = 123;
    program.process_cross_chain_message(message, nonce).unwrap();
    
    let result = program.process_cross_chain_message(message, nonce);
    assert_eq!(result.unwrap_err(), ErrorCode::InvalidNonce);
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_full_cross_chain_flow() {
    // Test complete cross-chain transfer flow
    let client = setup_test_client().await;
    
    // 1. Mint NFT
    let token_id = mint_test_nft(&client).await?;
    
    // 2. Send cross-chain
    let tx = client.send_nft_cross_chain(token_id, ETH_CHAIN_ID, eth_address).await?;
    
    // 3. Verify state changes
    let state = client.get_cross_chain_status(token_id).await?;
    assert_eq!(state.status, TransferStatus::Pending);
    
    // 4. Simulate completion
    simulate_cross_chain_completion(&client, token_id).await?;
    
    let final_state = client.get_cross_chain_status(token_id).await?;
    assert_eq!(final_state.status, TransferStatus::Completed);
}
```

### Fuzzing Tests
```rust
use arbitrary::Arbitrary;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    token_id: u64,
    chain_id: u64,
    address: [u8; 20],
    message: Vec<u8>,
}

#[test]
fn fuzz_cross_chain_message() {
    bolero::check!()
        .with_type::<FuzzInput>()
        .cloned()
        .for_each(|input| {
            let result = validate_cross_chain_message(&input.message);
            // Should never panic, only return Ok/Err
        });
}
```

## 🚀 Deployment Security

### Pre-Deployment

1. **Code Review**: Complete security review
2. **Static Analysis**: Use tools like Clippy, Rust analyzer
3. **Dependency Audit**: Check for vulnerable dependencies
4. **Test Coverage**: Ensure >95% test coverage

### Deployment Process

1. **Testnet Deployment**: Deploy to devnet first
2. **Security Testing**: Run full security test suite
3. **Bug Bounty**: Consider running a bug bounty program
4. **Gradual Rollout**: Start with limited functionality

### Post-Deployment

1. **Monitoring**: Set up comprehensive monitoring
2. **Incident Response**: Have emergency procedures ready
3. **Regular Audits**: Schedule periodic security reviews
4. **Updates**: Maintain and update regularly

## 📞 Security Contact

For security issues or concerns:

- **Email**: security@yourproject.com
- **Discord**: #security channel
- **Bug Bounty**: Link to bug bounty program

## 🔗 External Security Resources

- [Solana Security Best Practices](https://docs.solana.com/developing/programming-model/calling-between-programs#security)
- [Anchor Security Guidelines](https://book.anchor-lang.com/anchor_in_depth/program_security.html)
- [ZetaChain Security Documentation](https://docs.zetachain.com/developers/security/)
- [Cross-Chain Security Considerations](https://blog.zetachain.com/security-considerations-for-cross-chain-applications/)

---

**Remember: Security is an ongoing process, not a destination. Stay vigilant and keep improving!** 🛡️