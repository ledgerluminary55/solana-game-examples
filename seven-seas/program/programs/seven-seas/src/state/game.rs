use anchor_lang::prelude::*;
pub use crate::errors::TinyAdventureError;
use crate::CHEST_REWARD;
use crate::PLAYER_KILL_REWARD;
const BOARD_SIZE_X: usize = 10;
const BOARD_SIZE_Y: usize = 10;

const STATE_EMPTY: u8 = 0;
const STATE_PLAYER: u8 = 1;
const STATE_CHEST: u8 = 2;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    // We must specify the space in order to initialize an account.
    // First 8 bytes are default account discriminator,
    #[account(
        init,
        payer = signer, 
        seeds = [b"level"],
        bump,
        space = 10240
    )]
    pub new_game_data_account: AccountLoader<'info, GameDataAccount>,
    // This is the PDA in which we will deposit the reward SOl and
    // from where we send it back to the first player reaching the chest.
    #[account(
        init,
        seeds = [b"chestVault"],
        bump,
        payer = signer,
        space = 8
    )]
    pub chest_vault: Box<Account<'info, ChestVaultAccount>>,
    #[account(
        init,
        seeds = [b"gameActions"],
        bump,
        payer = signer,
        space = 4096
    )]
    pub game_actions: Box<Account<'info, GameActionHistory>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Reset<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    // We must specify the space in order to initialize an account.
    // First 8 bytes are default account discriminator,
    #[account(
        seeds = [b"level"],
        bump,
    )]
    pub new_game_data_account: AccountLoader<'info, GameDataAccount>,
    // This is the PDA in which we will deposit the reward SOl and
    // from where we send it back to the first player reaching the chest.
    #[account(
        seeds = [b"chestVault"],
        bump,
    )]
    pub chest_vault: Box<Account<'info, ChestVaultAccount>>,
    #[account(
        seeds = [b"gameActions"],
        bump,
    )]
    pub game_actions: Box<Account<'info, GameActionHistory>>,
    pub system_program: Program<'info, System>,
}

#[account(zero_copy)]
#[repr(packed)]
#[derive(Default)]
pub struct GameDataAccount {
    board: [[Tile; BOARD_SIZE_X]; BOARD_SIZE_Y],
    action_id: u64,
}

#[account]
pub struct GameActionHistory {
    id_counter: u64,
    game_actions: Vec<GameAction>,
}

// TODO: Do we need a ship pda that can be upgraded? :thinking: 
/*#[account]
pub struct Ship {
    health: u16,
    kills: u16,
    cannons: u16,
    upgrades: u16,
    xp: u16,
    level: u16
}*/

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct GameAction {
    action_id: u64,        // 1
    action_type: u8,      // 1
    player: Pubkey,      // 32
    target: Pubkey,      // 32
    damage: u64,         // 8   
}

#[zero_copy]
#[repr(packed)]
#[derive(Default)]
pub struct Tile {
    player: Pubkey,      // 32
    state: u8,           // 1
    health: u16,         // 1
    collect_reward: u64, // 8
    avatar: Pubkey,      // 32
    kills: u8,           // 1
    look_direction: u8,  // 1 (Up, right, down, left) 
}

impl GameDataAccount {
    pub fn print(&mut self) -> Result<()> {
        // print will only work locally for debugging otherwise its eats too muc compute
        
        /* 
        for x in 0..BOARD_SIZE_X {
            for y in 0..BOARD_SIZE_Y {
                let tile = self.board[x][y  ];
                if tile.state == STATE_EMPTY {
                    msg!("empty")
                } else {
                    msg!("{} {}", tile.player, tile.state)
                }
            }
        }*/

        Ok(())
    }

    pub fn shoot(
        &mut self,
        player: AccountInfo,
        game_actions: &mut GameActionHistory,
        chest_vault: AccountInfo
    ) -> Result<()> {

        let option_add = self.action_id.checked_add(1);
        match option_add {
            Some(val) =>  {
                self.action_id = val;
            }, 
            None => {
                self.action_id = 0;
            }
        }
        let item = GameAction {
            action_id: self.action_id,
            action_type: 0,
            player: player.key(),
            target: player.key(),
            damage: 5
        };

        if game_actions.game_actions.len() > 10 {
            game_actions.game_actions.drain(0..5);
        }

        game_actions.game_actions.push(item);

        let mut player_position: Option<(usize, usize)> = None;

        // Find the player on the board
        for x in 0..BOARD_SIZE_X {
            for y in 0..BOARD_SIZE_Y {
                let tile = self.board[x][y];
                if tile.state == STATE_PLAYER {
                    if tile.player == player.key.clone() {
                        player_position = Some((x, y));
                    }
                    msg!("{} {}", tile.player, tile.state);
                }
            }
        }

        // If the player is on the board move him
        match player_position {
            None => {
                return Err(TinyAdventureError::TriedToMovePlayerThatWasNotOnTheBoard.into());
            }
            Some(val) => {
                
                msg!("Player position x:{} y:{}", val.0, val.1);
                // TODO: use damage from a new BoatPDA 
                if val.0 < BOARD_SIZE_X -1 {
                    self.attackTile(( val.0 + 1, val.1), 50, player.clone(), chest_vault.clone())?;
                }
                
                if val.1 < BOARD_SIZE_Y -1 {
                    self.attackTile(( val.0, val.1 + 1), 50, player.clone(), chest_vault.clone())?;
                }
                
                if val.0 > 0 {
                    self.attackTile(( val.0 - 1, val.1), 50, player.clone(), chest_vault.clone())?;
                }
                
                if val.0 > 0 {
                    self.attackTile(( val.0, val.1  - 1), 50, player.clone(), chest_vault.clone())?;
                }
                
                
            }
        }

        Ok(())
    }

    fn attackTile(&mut self, val: (usize, usize), damage: u16, attacker: AccountInfo, chest_vault: AccountInfo)  -> Result<()> {
        let mut tile = self.board[val.0][val.1];
        msg!("Attack x:{} y:{}", val.0, val.1);

        if tile.state == STATE_PLAYER {
            let matchOption = tile.health.checked_sub(damage);
            match  matchOption {
                None => {
                    tile.health = 0;
                    msg!("Enemy killed x:{} y:{} pubkey: {}", val.0, val.1, tile.player);
                    self.board[val.0][val.1].state = STATE_EMPTY;
                    **chest_vault.try_borrow_mut_lamports()? -= tile.collect_reward;
                    **attacker.try_borrow_mut_lamports()? += tile.collect_reward;                },
                Some(value) =>  {
                    tile.health = value;
                }   
            };
        }
        Ok(())
    }

    pub fn move_in_direction(
        &mut self,
        direction: u8,
        player: AccountInfo,
        chest_vault: AccountInfo,
    ) -> Result<()> {
        let mut player_position: Option<(usize, usize)> = None;

        // Find the player on the board
        for x in 0..BOARD_SIZE_X {
            for y in 0..BOARD_SIZE_Y {
                let tile = self.board[x][y];
                if tile.state == STATE_PLAYER {
                    if tile.player == player.key.clone() {
                        player_position = Some((x, y));
                    }
                    // Printing the whole board eats too much compute
                    //msg!("{} {}", tile.player, tile.state);
                }
            }
        }

        // If the player is on the board move him
        match player_position {
            None => {
                return Err(TinyAdventureError::TriedToMovePlayerThatWasNotOnTheBoard.into());
            }
            Some(val) => {
                let mut new_player_position: (usize, usize) = (val.0, val.1);
                match direction {
                    // Up
                    0 => {
                        if new_player_position.1 == 0 {
                            new_player_position.1 = BOARD_SIZE_Y - 1;
                        } else {
                            new_player_position.1 -= 1;
                        }                    
                    }
                    // Right
                    1 => {
                        if new_player_position.0 == BOARD_SIZE_X - 1 {
                            new_player_position.0 = 0;
                        } else {
                            new_player_position.0 += 1;
                        }                        
                    }
                    // Down
                    2 => {
                        if new_player_position.1 == BOARD_SIZE_Y - 1 {
                            new_player_position.1 = 0;
                        } else {
                            new_player_position.1 += 1;
                        }                    
                    }
                    // Left
                    3 => {
                        if new_player_position.0 == 0 {
                            new_player_position.0 = BOARD_SIZE_X -1;
                        } else {
                            new_player_position.0 -= 1;
                        }                        
                    }
                    _ => {
                        return Err(TinyAdventureError::WrongDirectionInput.into());
                    }
                }

                let tile = self.board[new_player_position.0][new_player_position.1];
                if tile.state == STATE_EMPTY {
                    self.board[new_player_position.0][new_player_position.1] =
                        self.board[player_position.unwrap().0][player_position.unwrap().1];
                    self.board[player_position.unwrap().0][player_position.unwrap().1].state =
                        STATE_EMPTY;
                    msg!("Moved player to new tile");
                } else {
                    msg!(
                        "player position {} {}",
                        player_position.unwrap().0,
                        player_position.unwrap().1
                    );
                    msg!(
                        "new player position {} {}",
                        new_player_position.0,
                        new_player_position.1
                    );
                    if tile.state == STATE_CHEST {
                        self.board[new_player_position.0][new_player_position.1] =
                            self.board[player_position.unwrap().0][player_position.unwrap().1];
                        self.board[player_position.unwrap().0][player_position.unwrap().1].state =
                            STATE_EMPTY;
                        **chest_vault.try_borrow_mut_lamports()? -= tile.collect_reward;
                        **player.try_borrow_mut_lamports()? += tile.collect_reward;
                        msg!("Collected Chest");
                    }
                    if tile.state == STATE_PLAYER {
                        self.board[new_player_position.0][new_player_position.1] =
                            self.board[player_position.unwrap().0][player_position.unwrap().1];
                        self.board[player_position.unwrap().0][player_position.unwrap().1].state =
                            STATE_EMPTY;
                        **chest_vault.try_borrow_mut_lamports()? -= tile.collect_reward;
                        **player.try_borrow_mut_lamports()? += tile.collect_reward;
                        msg!("Other player killed");
                    }

                    msg!("{} type {}", tile.player, tile.state);
                }
            }
        }

        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        for x in 0..BOARD_SIZE_X {
            for y in 0..BOARD_SIZE_Y {
                self.board[x][y].state = STATE_EMPTY;
            }
        }
        Ok(())
    }

    pub fn spawn_player(&mut self, player: AccountInfo, avatar: Pubkey) -> Result<()> {
        let mut empty_slots: Vec<(usize, usize)> = Vec::new();

        for x in 0..BOARD_SIZE_X {
            for y in 0..BOARD_SIZE_Y {
                let tile = self.board[x][y];
                if tile.state == STATE_EMPTY {
                    empty_slots.push((x, y));
                } else {
                    if tile.player == player.key.clone() && tile.state == STATE_PLAYER {
                        return Err(TinyAdventureError::PlayerAlreadyExists.into());
                    }
                    //msg!("{}", tile.player);
                }
            }
        }

        if empty_slots.len() == 0 {
            return Err(TinyAdventureError::BoardIsFull.into());
        }

        let mut rng = XorShift64 {
            a: empty_slots.len() as u64,
        };

        let random_empty_slot = empty_slots[(rng.next() % (empty_slots.len() as u64)) as usize];
        msg!(
            "Player spawn at {} {}",
            random_empty_slot.0,
            random_empty_slot.1
        );
        self.board[random_empty_slot.0][random_empty_slot.1] = Tile {
            player: player.key.clone(),
            avatar: avatar.clone(),
            kills: 0,
            state: STATE_PLAYER,
            health: 1,
            collect_reward: PLAYER_KILL_REWARD,
        };

        Ok(())
    }

    pub fn spawn_chest(&mut self, player: AccountInfo) -> Result<()> {
        let mut empty_slots: Vec<(usize, usize)> = Vec::new();

        for x in 0..BOARD_SIZE_X {
            for y in 0..BOARD_SIZE_Y {
                let tile = self.board[x][y];
                if tile.state == STATE_EMPTY {
                    empty_slots.push((x, y));
                } else {
                    //msg!("{}", tile.player);
                }
            }
        }

        if empty_slots.len() == 0 {
            return Err(TinyAdventureError::BoardIsFull.into());
        }

        let mut rng = XorShift64 {
            a: (empty_slots.len() + 1) as u64,
        };

        let random_empty_slot = empty_slots[(rng.next() % (empty_slots.len() as u64)) as usize];
        msg!(
            "Chest spawn at {} {}",
            random_empty_slot.0,
            random_empty_slot.1
        );

        self.board[random_empty_slot.0][random_empty_slot.1] = Tile {
            player: player.key.clone(),
            avatar: player.key.clone(),
            kills: 0,
            state: STATE_CHEST,
            health: 1,
            collect_reward: CHEST_REWARD,
        };

        Ok(())
    }
}

#[derive(Accounts)]
pub struct MovePlayer<'info> {
    /// CHECK:
    #[account(mut)]
    pub chest_vault: AccountInfo<'info>,
    #[account(mut)]
    pub game_data_account: AccountLoader<'info, GameDataAccount>,
    /// CHECK:
    #[account(mut)]
    pub player: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Shoot<'info> {
    /// CHECK:
    #[account(mut)]
    pub chest_vault: AccountInfo<'info>,
    #[account(mut)]
    pub game_data_account: AccountLoader<'info, GameDataAccount>,
    #[account(mut)]
    pub game_actions: Account<'info, GameActionHistory>,
    /// CHECK:
    #[account(mut)]
    pub player: Signer<'info>,
}

#[derive(Accounts)]
pub struct SpawnPlayer<'info> {
    /// CHECK:
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK:
    #[account(mut)]
    pub chest_vault: AccountInfo<'info>,
    #[account(mut)]
    pub game_data_account: AccountLoader<'info, GameDataAccount>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct ChestVaultAccount {}

pub struct XorShift64 {
    a: u64,
}

impl XorShift64 {
    pub fn next(&mut self) -> u64 {
        let mut x = self.a;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.a = x;
        x
    }
}
