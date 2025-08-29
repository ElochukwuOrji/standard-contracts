use anchor_lang::prelude::*;
use anchor_spl::{
    metadata::{
        create_metadata_accounts_v3, 
        update_metadata_accounts_v2,
        CreateMetadataAccountsV3, 
        UpdateMetadataAccountsV2, 
        Metadata,
        MetadataAccount,
    },
    token::{self, Mint, MintTo, Token, TokenAccount, Transfer},
    associated_token::AssociatedToken,
};

pub mod gateway;

use mpl_token_metadata::{
    types::{DataV2, Creator, Collection},
};

declare_id!("D9Wnf46z72Wq6g7s4X7KZ8u6y6LmpDrDYKaLi6jHKdZr");

const AUTHORITY_SEED: &[u8] = b"authority";
const NFT_COLLECTION_SEED: &[u8] = b"nft_collection";
const NFT_MINT_SEED: &[u8] = b"nft_mint";
const CROSS_CHAIN_STATE_SEED: &[u8] = b"cross_chain_state";

// REAL ZetaChain Gateway Program Address (all networks: mainnet, testnet, devnet)
const ZETACHAIN_GATEWAY: Pubkey = anchor_lang::solana_program::pubkey!("ZETAjseVjuFsxdRxo6MmTCvqFwb3ZHUx56Co3vCmGis");

// ZetaChain Gateway PDA addresses
const GATEWAY_PDA_SEED: &[u8] = b"meta";
const GATEWAY_PDA: Pubkey = anchor_lang::solana_program::pubkey!("2f9SLuUNb7TNeM6gzBwT4ZjbL5ZyKzzHg1Ce9yiquEjj");
const GATEWAY_RENT_PAYER_PDA: Pubkey = anchor_lang::solana_program::pubkey!("Am1aA3XQciu3vMG6E9yLa2Y9TcTf2XB3D3akLtjVzu3L");

#[program]
pub mod solana_universal_nft {
    use super::*;

    /// Initialize the universal NFT program
    pub fn initialize(
        ctx: Context<Initialize>, 
        name: String,
        symbol: String,
        base_uri: String,
    ) -> Result<()> {
        let nft_collection = &mut ctx.accounts.nft_collection;
        nft_collection.authority = ctx.accounts.authority.key();
        nft_collection.name = name;
        nft_collection.symbol = symbol;
        nft_collection.base_uri = base_uri;
        nft_collection.total_supply = 0;
        nft_collection.bump = ctx.bumps.nft_collection;

        msg!("Universal NFT Collection initialized");
        Ok(())
    }

    /// Mint a new NFT locally on Solana
    pub fn mint_nft(
        ctx: Context<MintNft>,
        token_id: u64,
        name: String,
        uri: String,
        creator_fee: u16, // Basis points (e.g., 500 = 5%)
    ) -> Result<()> {
        let collection = &mut ctx.accounts.nft_collection;
        
        // Ensure token ID is unique
        require!(token_id > collection.total_supply, ErrorCode::InvalidTokenId);
        
        // Mint the NFT token
        let seeds = &[
            AUTHORITY_SEED,
            &[ctx.bumps.authority]
        ];
        let signer = &[&seeds[..]];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.nft_mint.to_account_info(),
                    to: ctx.accounts.nft_token_account.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
                signer,
            ),
            1, // NFTs have supply of 1
        )?;

        // Create metadata
        let creators = vec![Creator {
            address: ctx.accounts.owner.key(),
            verified: false,
            share: 100,
        }];

        let data = DataV2 {
            name,
            symbol: collection.symbol.clone(),
            uri,
            seller_fee_basis_points: creator_fee,
            creators: Some(creators),
            collection: Some(Collection {
                verified: false,
                key: collection.key(),
            }),
            uses: None,
        };

        create_metadata_accounts_v3(
            CpiContext::new_with_signer(
                ctx.accounts.metadata_program.to_account_info(),
                CreateMetadataAccountsV3 {
                    metadata: ctx.accounts.nft_metadata.to_account_info(),
                    mint: ctx.accounts.nft_mint.to_account_info(),
                    mint_authority: ctx.accounts.authority.to_account_info(),
                    update_authority: ctx.accounts.authority.to_account_info(),
                    payer: ctx.accounts.owner.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                signer,
            ),
            data,
            true, // is_mutable
            true, // update_authority_is_signer
            None, // collection_details
        )?;

        collection.total_supply += 1;

        msg!("NFT minted with token ID: {}", token_id);
        Ok(())
    }

    /// Send NFT to another chain via ZetaChain Gateway
    pub fn send_nft_cross_chain(
        ctx: Context<SendNftCrossChain>,
        token_id: u64,
        destination_chain_id: u64,
        destination_address: [u8; 20],
        revert_options: Option<RevertOptions>,
    ) -> Result<()> {
        // Verify ownership
        require!(
            ctx.accounts.nft_token_account.amount == 1,
            ErrorCode::NotOwner
        );

        // Create cross-chain state to track the transfer
        let cross_chain_state = &mut ctx.accounts.cross_chain_state;
        cross_chain_state.token_id = token_id;
        cross_chain_state.original_owner = ctx.accounts.owner.key();
        cross_chain_state.destination_chain_id = destination_chain_id;
        cross_chain_state.destination_address = destination_address;
        cross_chain_state.status = TransferStatus::Pending;
        cross_chain_state.bump = ctx.bumps.cross_chain_state;

        // Transfer NFT to program custody
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.nft_token_account.to_account_info(),
                    to: ctx.accounts.program_nft_account.to_account_info(),
                    authority: ctx.accounts.owner.to_account_info(),
                },
            ),
            1,
        )?;

        // Prepare NFT data for cross-chain transfer
        let nft_data = NftTransferData {
            token_id,
            name: ctx.accounts.nft_metadata.name.clone(),
            symbol: ctx.accounts.nft_metadata.symbol.clone(),
            uri: ctx.accounts.nft_metadata.uri.clone(),
            creator_fee: ctx.accounts.nft_metadata.seller_fee_basis_points,
        };

        let message = nft_data.try_to_vec()?;

        // Call ZetaChain Gateway deposit instruction for cross-chain transfer
        let gateway_instruction = create_gateway_deposit_instruction(
            &ctx.accounts.gateway_program.key(),
            &ctx.accounts.gateway_pda.key(),
            &ctx.accounts.owner.key(),
            0, // amount in lamports (0 for NFT data transfer)
            destination_address,
            message,
            revert_options,
        )?;

        anchor_lang::solana_program::program::invoke(
            &gateway_instruction,
            &[
                ctx.accounts.gateway_program.to_account_info(),
                ctx.accounts.gateway_pda.to_account_info(),
                ctx.accounts.owner.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        msg!(
            "NFT {} sent to chain {} at address {:?}",
            token_id,
            destination_chain_id,
            destination_address
        );

        Ok(())
    }

    /// Receive and mint incoming NFT from another chain
    /// This is called by ZetaChain when an NFT is sent to Solana
    pub fn on_call(
        ctx: Context<OnCall>,
        sender: [u8; 20],
        data: Vec<u8>,
    ) -> Result<()> {
        msg!("Receiving cross-chain NFT call from sender: {:?}", sender);

        // Decode the NFT data
        let nft_data: NftTransferData = NftTransferData::try_from_slice(&data)
            .map_err(|_| ErrorCode::InvalidNftData)?;

        // Generate new mint for the incoming NFT
        let seeds = &[
            NFT_MINT_SEED,
            &nft_data.token_id.to_le_bytes(),
            &[ctx.bumps.nft_mint]
        ];
        let signer = &[&seeds[..]];

        // Mint the NFT
        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.nft_mint.to_account_info(),
                    to: ctx.accounts.recipient_token_account.to_account_info(),
                    authority: ctx.accounts.nft_mint.to_account_info(),
                },
                signer,
            ),
            1,
        )?;

        // Create metadata for the incoming NFT
        // Convert sender address to a dummy pubkey for creator
        let creator_key = Pubkey::new_from_array([
            sender[0], sender[1], sender[2], sender[3],
            sender[4], sender[5], sender[6], sender[7],
            sender[8], sender[9], sender[10], sender[11],
            sender[12], sender[13], sender[14], sender[15],
            sender[16], sender[17], sender[18], sender[19],
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]);

        let creators = vec![Creator {
            address: creator_key,
            verified: false,
            share: 100,
        }];

        let metadata_data = DataV2 {
            name: nft_data.name,
            symbol: nft_data.symbol,
            uri: nft_data.uri,
            seller_fee_basis_points: nft_data.creator_fee,
            creators: Some(creators),
            collection: Some(Collection {
                verified: false,
                key: ctx.accounts.nft_collection.key(),
            }),
            uses: None,
        };

        create_metadata_accounts_v3(
            CpiContext::new_with_signer(
                ctx.accounts.metadata_program.to_account_info(),
                CreateMetadataAccountsV3 {
                    metadata: ctx.accounts.nft_metadata.to_account_info(),
                    mint: ctx.accounts.nft_mint.to_account_info(),
                    mint_authority: ctx.accounts.nft_mint.to_account_info(),
                    update_authority: ctx.accounts.authority.to_account_info(),
                    payer: ctx.accounts.recipient.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                signer,
            ),
            metadata_data,
            true,
            false, // update_authority_is_signer = false since we use program authority
            None,
        )?;

        let collection = &mut ctx.accounts.nft_collection;
        collection.total_supply += 1;

        msg!("Cross-chain NFT {} received and minted", nft_data.token_id);
        Ok(())
    }

    /// Handle revert scenarios for failed cross-chain transfers
    pub fn on_revert(
        ctx: Context<OnRevert>,
        sender: Pubkey,
        data: Vec<u8>,
    ) -> Result<()> {
        msg!("Reverting cross-chain NFT transfer");

        let cross_chain_state = &mut ctx.accounts.cross_chain_state;
        
        // Verify this is a legitimate revert
        require!(
            cross_chain_state.original_owner == sender,
            ErrorCode::UnauthorizedRevert
        );
        require!(
            cross_chain_state.status == TransferStatus::Pending,
            ErrorCode::InvalidRevertState
        );

        // Return NFT to original owner
        let seeds = &[
            AUTHORITY_SEED,
            &[ctx.bumps.authority]
        ];
        let signer = &[&seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_nft_account.to_account_info(),
                    to: ctx.accounts.owner_token_account.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
                signer,
            ),
            1,
        )?;

        cross_chain_state.status = TransferStatus::Reverted;

        msg!("NFT {} reverted to owner", cross_chain_state.token_id);
        Ok(())
    }

    /// Update NFT metadata (only by authority)
    pub fn update_nft_metadata(
        ctx: Context<UpdateNftMetadata>,
        token_id: u64,
        new_uri: Option<String>,
        new_name: Option<String>,
    ) -> Result<()> {
        let current_metadata = &ctx.accounts.nft_metadata;
        
        let data = DataV2 {
            name: new_name.unwrap_or(current_metadata.name.clone()),
            symbol: current_metadata.symbol.clone(),
            uri: new_uri.unwrap_or(current_metadata.uri.clone()),
            seller_fee_basis_points: current_metadata.seller_fee_basis_points,
            creators: current_metadata.creators.clone(),
            collection: current_metadata.collection.clone(),
            uses: current_metadata.uses.clone(),
        };

        let seeds = &[
            AUTHORITY_SEED,
            &[ctx.bumps.program_authority]
        ];
        let signer = &[&seeds[..]];

        update_metadata_accounts_v2(
            CpiContext::new_with_signer(
                ctx.accounts.metadata_program.to_account_info(),
                UpdateMetadataAccountsV2 {
                    metadata: ctx.accounts.nft_metadata.to_account_info(),
                    update_authority: ctx.accounts.program_authority.to_account_info(),
                },
                signer,
            ),
            None, // new_update_authority
            Some(data),
            None, // new_primary_sale_happened
            Some(true), // is_mutable
        )?;

        msg!("NFT {} metadata updated", token_id);
        Ok(())
    }
}

// Account structures
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + NftCollection::INIT_SPACE,
        seeds = [NFT_COLLECTION_SEED],
        bump
    )]
    pub nft_collection: Account<'info, NftCollection>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(token_id: u64)]
pub struct MintNft<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [NFT_COLLECTION_SEED],
        bump = nft_collection.bump
    )]
    pub nft_collection: Account<'info, NftCollection>,

    #[account(
        seeds = [AUTHORITY_SEED],
        bump
    )]
    /// CHECK: PDA for mint authority
    pub authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = owner,
        mint::decimals = 0,
        mint::authority = authority,
        seeds = [NFT_MINT_SEED, &token_id.to_le_bytes()],
        bump
    )]
    pub nft_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = owner,
        associated_token::mint = nft_mint,
        associated_token::authority = owner
    )]
    pub nft_token_account: Account<'info, TokenAccount>,

    /// CHECK: Metadata account will be created by Metaplex
    #[account(
        mut,
        seeds = [
            b"metadata",
            metadata_program.key().as_ref(),
            nft_mint.key().as_ref()
        ],
        seeds::program = metadata_program.key(),
        bump
    )]
    pub nft_metadata: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub metadata_program: Program<'info, Metadata>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(token_id: u64)]
pub struct SendNftCrossChain<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [AUTHORITY_SEED],
        bump
    )]
    /// CHECK: Program authority PDA
    pub authority: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = owner
    )]
    pub nft_token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = nft_mint,
        associated_token::authority = authority
    )]
    pub program_nft_account: Account<'info, TokenAccount>,

    #[account(
        seeds = [NFT_MINT_SEED, &token_id.to_le_bytes()],
        bump
    )]
    pub nft_mint: Account<'info, Mint>,

    #[account(
        seeds = [
            b"metadata",
            metadata_program.key().as_ref(),
            nft_mint.key().as_ref()
        ],
        seeds::program = metadata_program.key(),
        bump
    )]
    pub nft_metadata: Account<'info, MetadataAccount>,

    #[account(
        init,
        payer = owner,
        space = 8 + CrossChainState::INIT_SPACE,
        seeds = [CROSS_CHAIN_STATE_SEED, &token_id.to_le_bytes()],
        bump
    )]
    pub cross_chain_state: Account<'info, CrossChainState>,

    /// CHECK: ZetaChain Gateway Program - verified by address
    #[account(address = ZETACHAIN_GATEWAY)]
    pub gateway_program: UncheckedAccount<'info>,
    
    /// CHECK: Gateway PDA account - verified by address
    #[account(address = GATEWAY_PDA)]
    pub gateway_pda: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub metadata_program: Program<'info, Metadata>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct OnCall<'info> {
    #[account(mut)]
    pub recipient: Signer<'info>,

    #[account(
        mut,
        seeds = [NFT_COLLECTION_SEED],
        bump = nft_collection.bump
    )]
    pub nft_collection: Account<'info, NftCollection>,

    #[account(
        seeds = [AUTHORITY_SEED],
        bump
    )]
    /// CHECK: Program authority PDA
    pub authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = recipient,
        mint::decimals = 0,
        mint::authority = nft_mint,
        seeds = [NFT_MINT_SEED, &nft_collection.total_supply.to_le_bytes()],
        bump
    )]
    pub nft_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = recipient,
        associated_token::mint = nft_mint,
        associated_token::authority = recipient
    )]
    pub recipient_token_account: Account<'info, TokenAccount>,

    /// CHECK: Metadata account will be created
    #[account(
        mut,
        seeds = [
            b"metadata",
            metadata_program.key().as_ref(),
            nft_mint.key().as_ref()
        ],
        seeds::program = metadata_program.key(),
        bump
    )]
    pub nft_metadata: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub metadata_program: Program<'info, Metadata>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct OnRevert<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [AUTHORITY_SEED],
        bump
    )]
    /// CHECK: Program authority PDA
    pub authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [CROSS_CHAIN_STATE_SEED, &cross_chain_state.token_id.to_le_bytes()],
        bump = cross_chain_state.bump
    )]
    pub cross_chain_state: Account<'info, CrossChainState>,

    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = authority
    )]
    pub program_nft_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = owner
    )]
    pub owner_token_account: Account<'info, TokenAccount>,

    pub nft_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(token_id: u64)]
pub struct UpdateNftMetadata<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [AUTHORITY_SEED],
        bump
    )]
    /// CHECK: Program authority PDA
    pub program_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [NFT_MINT_SEED, &token_id.to_le_bytes()],
        bump
    )]
    pub nft_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [
            b"metadata",
            metadata_program.key().as_ref(),
            nft_mint.key().as_ref()
        ],
        seeds::program = metadata_program.key(),
        bump
    )]
    pub nft_metadata: Account<'info, MetadataAccount>,

    pub metadata_program: Program<'info, Metadata>,
}

// Data structures
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

#[account]
#[derive(InitSpace)]
pub struct CrossChainState {
    pub token_id: u64,
    pub original_owner: Pubkey,
    pub destination_chain_id: u64,
    pub destination_address: [u8; 20],
    pub status: TransferStatus,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub enum TransferStatus {
    Pending,
    Completed,
    Reverted,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct NftTransferData {
    pub token_id: u64,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub creator_fee: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RevertOptions {
    pub revert_address: Pubkey,
    pub abort_address: [u8; 20],
    pub call_on_revert: bool,
    pub revert_message: Vec<u8>,
    pub on_revert_gas_limit: u64,
}

// Error codes
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
    #[msg("Invalid NFT data format")]
    InvalidNftData,
    #[msg("Gateway call failed")]
    GatewayCallFailed,
    #[msg("Invalid TSS signature")]
    InvalidTssSignature,
}

/// Create ZetaChain Gateway deposit instruction for cross-chain NFT transfer
fn create_gateway_deposit_instruction(
    gateway_program: &Pubkey,
    gateway_pda: &Pubkey,
    payer: &Pubkey,
    amount: u64, // 0 for NFT data transfer
    receiver: [u8; 20],
    nft_data: Vec<u8>,
    revert_options: Option<RevertOptions>,
) -> Result<anchor_lang::solana_program::instruction::Instruction> {
    use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
    
    // Create the deposit instruction data according to ZetaChain Gateway format
    let mut instruction_data = vec![
        // Instruction discriminator for 'deposit'
        // This should match the actual discriminator from ZetaChain Gateway IDL
        242, 35, 198, 137, 82, 225, 242, 182, // deposit instruction discriminator
    ];
    
    // Serialize parameters
    instruction_data.extend_from_slice(&amount.to_le_bytes());
    instruction_data.extend_from_slice(&receiver);
    
    // Add revert options if provided
    if let Some(options) = revert_options {
        instruction_data.push(1); // Some
        instruction_data.extend_from_slice(&options.try_to_vec().unwrap());
    } else {
        instruction_data.push(0); // None
    }
    
    // Add NFT data as additional context
    instruction_data.extend_from_slice(&nft_data);

    let accounts = vec![
        AccountMeta::new(*gateway_pda, false),
        AccountMeta::new(*payer, true),
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
    ];

    Ok(Instruction {
        program_id: *gateway_program,
        accounts,
        data: instruction_data,
    })
}