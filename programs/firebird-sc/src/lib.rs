use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

declare_id!("FSH9An6asnz4m4WdhUkmsCjTWh4Q3ytoa6mcEva6xYqZ"); // Replace with your program ID

const TRIGGER_ADDRESS: &str = "9s3TcTSpTXMzQ3RFW8GC97o9ooTe7ZRu6zPUai5NdUgf";
const RAYDIUM_PROGRAM_ADDRESS: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

// Define the swap instruction structure for Raydium
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaydiumSwapInstruction {
    pub instruction: u8,     // Instruction ID for Raydium (8 bits)
    pub amount_in: u64,      // Amount of tokens to swap (64 bits)
    pub min_amount_out: u64, // Minimum amount of output tokens (64 bits)
}

impl RaydiumSwapInstruction {
    pub fn to_bytes(&self) -> [u8; 17] {
        let mut bytes = [0u8; 17];
        
        bytes[0] = self.instruction;
        bytes[1..9].copy_from_slice(&self.amount_in.to_le_bytes());
        bytes[9..17].copy_from_slice(&self.min_amount_out.to_le_bytes());
        
        bytes
    }
}

#[program]
pub mod firebird_contract {

    use super::*;

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        let token_address = ctx.accounts.token_mint.key();

        // Ensure the pool hasn't already been registered for this token
        let dca_data = &mut ctx.accounts.dca_data;
        // if dca_data.token_address == token_address {
        //     return Err(ErrorCode::TokenAlreadyDeposited.into());
        // }

        // Transfer tokens from user to PDA
        let cpi_accounts = anchor_spl::token::Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.pda_token_account.to_account_info(),
            authority: ctx.accounts.user_authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        anchor_spl::token::transfer(cpi_ctx, amount)?;

        // new added
        if dca_data.token_address == token_address {
            // Accumulate the amount if the token has already been deposited
            dca_data.piece += amount / 100; 
        } else {
            // New token deposit: initialize data
            dca_data.token_address = token_address;
            dca_data.piece = amount / 100; 
        }

        // end new added

        // Save the DCA data
        // dca_data.token_address = token_address;
        // dca_data.piece = amount / 100; // Store piece as integer

        Ok(())
    }

    pub fn sell<'info>(ctx: Context<'_, '_, '_, 'info, Sell<'info>>) -> Result<()> {
        if ctx.accounts.user_authority.key().to_owned().to_string() != TRIGGER_ADDRESS {
            return Err(ErrorCode::InvalidTrigger.into());
        }

        if ctx.accounts.raydium_program.key().to_owned().to_string() != RAYDIUM_PROGRAM_ADDRESS {
            return Err(ErrorCode::InvalidRaydiumProgram.into());
        }

        let token_address = ctx.accounts.token_mint.key();

        let (_vault_pda, vault_bump) = Pubkey::find_program_address(
            &[b"vault", token_address.as_ref()],
            ctx.program_id,
        );
        let dca_data = &ctx.accounts.dca_data;

        // Check if the token_address has been registered
        if dca_data.token_address != token_address {
            return Err(ErrorCode::InvalidTokenAddress.into());
        }

        // Get the amount available in PDA
        let pda_balance = ctx.accounts.pda_token_account.amount;
        if pda_balance == 0 {
            return Err(ErrorCode::InsufficientFunds.into());
        }

        // Calculate the amount to sell (either `piece` or whatever is available)
        let amount_to_sell = std::cmp::min(dca_data.piece, pda_balance);

        // Define minimum amount out for slippage protection (for now, 1)
        let min_amount_out = 1;

        // Construct the Raydium swap instruction
        let swap_instruction_data = RaydiumSwapInstruction {
            instruction: 9, // Swap instruction ID for Raydium
            amount_in: amount_to_sell,      // Amount to swap
            min_amount_out: min_amount_out, // Minimum amount out for slippage protection
        };

        // Serialize the instruction data
        let swap_data_vec = swap_instruction_data.to_bytes().to_vec();

        // Construct Raydium swap instruction
        let ix = solana_program::instruction::Instruction {
            program_id: ctx.accounts.raydium_program.key(), // Raydium program ID
            accounts: vec![
                AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
                AccountMeta::new(ctx.remaining_accounts[0].key(), false),
                AccountMeta::new_readonly(ctx.remaining_accounts[1].key(), false),
                AccountMeta::new(ctx.remaining_accounts[2].key(), false),
                AccountMeta::new(ctx.remaining_accounts[3].key(), false),
                AccountMeta::new(ctx.remaining_accounts[4].key(), false),
                AccountMeta::new(ctx.remaining_accounts[5].key(), false),
                AccountMeta::new_readonly(ctx.remaining_accounts[6].key(), false),
                AccountMeta::new(ctx.remaining_accounts[7].key(), false),
                AccountMeta::new(ctx.remaining_accounts[8].key(), false),
                AccountMeta::new(ctx.remaining_accounts[9].key(), false),
                AccountMeta::new(ctx.remaining_accounts[10].key(), false),
                AccountMeta::new(ctx.remaining_accounts[11].key(), false),
                AccountMeta::new(ctx.remaining_accounts[12].key(), false),
                AccountMeta::new_readonly(ctx.remaining_accounts[13].key(), false),
                AccountMeta::new(ctx.accounts.pda_token_account.key(), false), // PDA token account (source)
                AccountMeta::new(ctx.accounts.pda_wsol_account.key(), false), // PDA token account (dest)
                AccountMeta::new(ctx.accounts.user_authority.key(), true)
            ],
            data: swap_data_vec, // Serialized Raydium swap data
        };

        // Perform CPI with the PDA signing
        let seeds: &[&[u8]] = &[
            b"vault",
            token_address.as_ref(),
            &[vault_bump],
        ];
        let signer = &[&seeds[..]];
        solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.raydium_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.remaining_accounts[0].to_account_info(),
                ctx.remaining_accounts[1].to_account_info(),
                ctx.remaining_accounts[2].to_account_info(),
                ctx.remaining_accounts[3].to_account_info(),
                ctx.remaining_accounts[4].to_account_info(),
                ctx.remaining_accounts[5].to_account_info(),
                ctx.remaining_accounts[6].to_account_info(),
                ctx.remaining_accounts[7].to_account_info(),
                ctx.remaining_accounts[8].to_account_info(),
                ctx.remaining_accounts[9].to_account_info(),
                ctx.remaining_accounts[10].to_account_info(),
                ctx.remaining_accounts[11].to_account_info(),
                ctx.remaining_accounts[12].to_account_info(),
                ctx.remaining_accounts[13].to_account_info(),
                ctx.accounts.pda_token_account.to_account_info(),
                ctx.accounts.pda_wsol_account.to_account_info(),
                ctx.accounts.user_authority.to_account_info()
            ],
            signer,
        )?;

        Ok(())
    }

    pub fn buy_back<'info>(ctx: Context<'_, '_, '_, 'info, BuyBack<'info>>, amount: u64) -> Result<()> {
        if ctx.accounts.user_authority.key().to_owned().to_string() != TRIGGER_ADDRESS {
            return Err(ErrorCode::InvalidTrigger.into());
        }

        if ctx.accounts.raydium_program.key().to_owned().to_string() != RAYDIUM_PROGRAM_ADDRESS {
            return Err(ErrorCode::InvalidRaydiumProgram.into());
        }
        
        let token_address = ctx.accounts.token_mint.key();

        let (_vault_wsol_pda, vault_wsol_bump) = Pubkey::find_program_address(
            &[b"vault-wsol", token_address.as_ref()],
            ctx.program_id,
        );

        // Get the amount available in PDA
        let wsol_balance = ctx.accounts.pda_wsol_account.amount;
        if wsol_balance < amount {
            return Err(ErrorCode::InsufficientFunds.into());
        }

        // Define minimum amount out for slippage protection (for now, 1)
        let min_amount_out = 1;

        // Construct the Raydium swap instruction
        let swap_instruction_data = RaydiumSwapInstruction {
            instruction: 9, // Swap instruction ID for Raydium
            amount_in: amount,      // Amount to swap
            min_amount_out: min_amount_out, // Minimum amount out for slippage protection
        };

        // Serialize the instruction data
        let swap_data_vec = swap_instruction_data.to_bytes().to_vec();

        // Construct Raydium swap instruction
        let ix = solana_program::instruction::Instruction {
            program_id: ctx.accounts.raydium_program.key(), // Raydium program ID
            accounts: vec![
                AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
                AccountMeta::new(ctx.remaining_accounts[0].key(), false),
                AccountMeta::new_readonly(ctx.remaining_accounts[1].key(), false),
                AccountMeta::new(ctx.remaining_accounts[2].key(), false),
                AccountMeta::new(ctx.remaining_accounts[3].key(), false),
                AccountMeta::new(ctx.remaining_accounts[4].key(), false),
                AccountMeta::new(ctx.remaining_accounts[5].key(), false),
                AccountMeta::new_readonly(ctx.remaining_accounts[6].key(), false),
                AccountMeta::new(ctx.remaining_accounts[7].key(), false),
                AccountMeta::new(ctx.remaining_accounts[8].key(), false),
                AccountMeta::new(ctx.remaining_accounts[9].key(), false),
                AccountMeta::new(ctx.remaining_accounts[10].key(), false),
                AccountMeta::new(ctx.remaining_accounts[11].key(), false),
                AccountMeta::new(ctx.remaining_accounts[12].key(), false),
                AccountMeta::new_readonly(ctx.remaining_accounts[13].key(), false),
                AccountMeta::new(ctx.accounts.pda_wsol_account.key(), false), // PDA token account (dest)
                AccountMeta::new(ctx.accounts.pda_token_account.key(), false), // PDA token account (source)
                AccountMeta::new(ctx.accounts.user_authority.key(), true)
            ],
            data: swap_data_vec, // Serialized Raydium swap data
        };

        // Perform CPI with the PDA signing
        let seeds: &[&[u8]] = &[
            b"vault-wsol",
            token_address.as_ref(),
            &[vault_wsol_bump],
        ];
        let signer = &[&seeds[..]];
        solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.raydium_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.remaining_accounts[0].to_account_info(),
                ctx.remaining_accounts[1].to_account_info(),
                ctx.remaining_accounts[2].to_account_info(),
                ctx.remaining_accounts[3].to_account_info(),
                ctx.remaining_accounts[4].to_account_info(),
                ctx.remaining_accounts[5].to_account_info(),
                ctx.remaining_accounts[6].to_account_info(),
                ctx.remaining_accounts[7].to_account_info(),
                ctx.remaining_accounts[8].to_account_info(),
                ctx.remaining_accounts[9].to_account_info(),
                ctx.remaining_accounts[10].to_account_info(),
                ctx.remaining_accounts[11].to_account_info(),
                ctx.remaining_accounts[12].to_account_info(),
                ctx.remaining_accounts[13].to_account_info(),
                ctx.accounts.pda_token_account.to_account_info(),
                ctx.accounts.pda_wsol_account.to_account_info(),
                ctx.accounts.user_authority.to_account_info()
            ],
            signer,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    pub token_mint: Account<'info, Mint>, // SPL token mint address

    #[account(
        init_if_needed,
        space = 8 + 32 + 64,
        payer = user_authority,
        seeds = [b"dca_data".as_ref(), token_mint.key().as_ref()],
        bump,
    )]
    pub dca_data: Box<Account<'info, DCAData>>, // Data account to store DCA info

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>, // User's SPL token account

    #[account(
        init_if_needed,
        seeds = [b"vault".as_ref(), token_mint.key().as_ref()],
        bump,
        payer = user_authority,
        token::mint = token_mint,
        token::authority = pda_token_account,
    )]
    pub pda_token_account: Account<'info, TokenAccount>, // PDA's SPL token account
    
    pub token_program: Program<'info, Token>, // Token program
    pub system_program: Program<'info, System>,

    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(mut)]
    pub user_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct Sell<'info> {
    pub token_mint: Box<Account<'info, Mint>>, // SPL token mint address
    pub dca_data: Box<Account<'info, DCAData>>, // DCA data

    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(mut)]
    pub raydium_program: AccountInfo<'info>,

    #[account(mut)]
    pub pda_token_account: Box<Account<'info, TokenAccount>>, // PDA's SPL token account

    #[account(
        init_if_needed,
        seeds = [b"vault-wsol".as_ref(), token_mint.key().as_ref()],
        bump,
        payer = user_authority,
        token::mint = pool_token_b,
        token::authority = pda_wsol_account,
    )]
    pub pda_wsol_account: Box<Account<'info, TokenAccount>>, // PDA's wrapped SOL account
    
    /// CHECK:` doc comment explaining why no checks through types are necessary.
    pub pool_token_b: AccountInfo<'info>,

    pub token_program: Program<'info, Token>, // Token program
    pub system_program: Program<'info, System>,

    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(mut)]
    pub user_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct BuyBack<'info> {
    pub token_mint: Box<Account<'info, Mint>>, // SPL token mint address

    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(mut)]
    pub raydium_program: AccountInfo<'info>,

    #[account(mut)]
    pub pda_token_account: Box<Account<'info, TokenAccount>>, // PDA's SPL token account

    /// CHECK:` doc comment explaining why no checks through types are necessary.
    pub pool_token_b: AccountInfo<'info>,

    #[account(
        init_if_needed,
        seeds = [b"vault-wsol".as_ref(), token_mint.key().as_ref()],
        bump,
        payer = user_authority,
        token::mint = pool_token_b,
        token::authority = user_authority,
    )]
    pub pda_wsol_account: Box<Account<'info, TokenAccount>>, // PDA's wrapped SOL account

    pub token_program: Program<'info, Token>, // Token program
    pub system_program: Program<'info, System>,

    /// CHECK:` doc comment explaining why no checks through types are necessary.
    #[account(mut)]
    pub user_authority: Signer<'info>,
}


#[account]
pub struct DCAData {
    pub token_address: Pubkey, // Registered SPL token address
    pub piece: u64,            // Amount to sell in each call
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid trigger")]
    InvalidTrigger,
    #[msg("Invalid raydium program address")]
    InvalidRaydiumProgram,
    #[msg("The token is already deposited")]
    TokenAlreadyDeposited,
    #[msg("Invalid token address")]
    InvalidTokenAddress,
    #[msg("Insufficient funds to sell")]
    InsufficientFunds,
}
