use actix::{Context, Handler, System};

use crate::actors::blocks_manager::{BlocksManager, BlocksManagerError};
use crate::actors::epoch_manager::messages::EpochNotification;

use witnet_data_structures::{
    chain::{Block, CheckpointBeacon, Hash, InvVector},
    error::{ChainInfoError, ChainInfoErrorKind, ChainInfoResult},
};

use witnet_util::error::WitnetError;

use log::{debug, error};

use super::messages::{AddNewBlock, GetBlock, GetBlocksEpochRange, GetHighestCheckpointBeacon};

use crate::actors::session::messages::AnnounceItems;
use crate::actors::sessions_manager::{messages::Broadcast, SessionsManager};

////////////////////////////////////////////////////////////////////////////////////////
// ACTOR MESSAGE HANDLERS
////////////////////////////////////////////////////////////////////////////////////////
/// Payload for the notification for a specific epoch
#[derive(Debug)]
pub struct EpochPayload;

/// Payload for the notification for all epochs
#[derive(Clone, Debug)]
pub struct EveryEpochPayload;

/// Handler for EpochNotification<EpochPayload>
impl Handler<EpochNotification<EpochPayload>> for BlocksManager {
    type Result = ();

    fn handle(&mut self, msg: EpochNotification<EpochPayload>, _ctx: &mut Context<Self>) {
        debug!("Epoch notification received {:?}", msg.checkpoint);
    }
}

/// Handler for EpochNotification<EveryEpochPayload>
impl Handler<EpochNotification<EveryEpochPayload>> for BlocksManager {
    type Result = ();

    fn handle(&mut self, msg: EpochNotification<EveryEpochPayload>, _ctx: &mut Context<Self>) {
        debug!("Periodic epoch notification received {:?}", msg.checkpoint);
    }
}

/// Handler for GetHighestBlockCheckpoint message
impl Handler<GetHighestCheckpointBeacon> for BlocksManager {
    type Result = ChainInfoResult<CheckpointBeacon>;

    fn handle(
        &mut self,
        _msg: GetHighestCheckpointBeacon,
        _ctx: &mut Context<Self>,
    ) -> Self::Result {
        if let Some(chain_info) = &self.chain_info {
            Ok(chain_info.highest_block_checkpoint)
        } else {
            error!("No ChainInfo loaded in BlocksManager");
            Err(WitnetError::from(ChainInfoError::new(
                ChainInfoErrorKind::ChainInfoNotFound,
                "No ChainInfo loaded in BlocksManager".to_string(),
            )))
        }
    }
}

/// Handler for AddNewBlock message
impl Handler<AddNewBlock> for BlocksManager {
    type Result = Result<Hash, BlocksManagerError>;

    fn handle(
        &mut self,
        msg: AddNewBlock,
        _ctx: &mut Context<Self>,
    ) -> Result<Hash, BlocksManagerError> {
        let res = self.process_new_block(msg.block);
        match res {
            Ok(hash) => {
                // Get SessionsManager's address
                let sessions_manager_addr = System::current().registry().get::<SessionsManager>();

                // Tell SessionsManager to announce the new block through every consolidated Session
                let items = vec![InvVector::Block(hash)];
                sessions_manager_addr.do_send(Broadcast {
                    command: AnnounceItems { items },
                });
            }
            Err(BlocksManagerError::BlockAlreadyExists) => {
                debug!("Block already exists");
            }
            Err(BlocksManagerError::StorageError(_)) => {
                debug!("Error when serializing block");
            }
            Err(_) => {
                debug!("Unexpected error");
            }
        };

        res
    }
}

/// Handler for GetBlock message
impl Handler<GetBlock> for BlocksManager {
    type Result = Result<Block, BlocksManagerError>;

    fn handle(
        &mut self,
        msg: GetBlock,
        _ctx: &mut Context<Self>,
    ) -> Result<Block, BlocksManagerError> {
        // Try to get block by hash
        self.try_to_get_block(msg.hash)
    }
}

/// Handler for GetBlocksEpochRange
impl Handler<GetBlocksEpochRange> for BlocksManager {
    type Result = Result<Vec<InvVector>, BlocksManagerError>;

    fn handle(
        &mut self,
        GetBlocksEpochRange { range }: GetBlocksEpochRange,
        _ctx: &mut Context<Self>,
    ) -> Self::Result {
        debug!("GetBlocksEpochRange received {:?}", range);
        let hashes = range
            .map(|epoch| &self.epoch_to_block_hash[&epoch])
            .flatten()
            .map(|hash| InvVector::Block(*hash))
            .collect();

        Ok(hashes)
    }
}
