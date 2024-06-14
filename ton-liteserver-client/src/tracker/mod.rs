use adnl_tcp::types::{Int, Long};

pub mod masterchain_last_block_tracker;
pub mod masterchain_first_block_tracker;
pub mod workchains_last_blocks_tracker;
pub mod workchains_first_blocks_tracker;
mod find_first_block;
mod masterchain_last_block_header_tracker;

type ShardId = (Int, Long);
