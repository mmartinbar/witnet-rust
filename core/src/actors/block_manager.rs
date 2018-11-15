use actix::{Actor, Context, Handler, SystemService};

use crate::actors::epoch_manager::{EpochNotification, EpochNotifiable};

use log::debug;

////////////////////////////////////////////////////////////////////////////////////////
// ACTOR BASIC STRUCTURE
////////////////////////////////////////////////////////////////////////////////////////
/// Block manager actor
#[derive(Default)]
pub struct BlockManager;

/// Make actor from `BlockManager`
impl Actor for BlockManager {
    /// Every actor has to provide execution `Context` in which it can run
    type Context = Context<Self>;

    /// Method to be executed when the actor is started
    fn started(&mut self, _ctx: &mut Self::Context) {
        debug!("Block Manager actor has been started!");
    }
}

/// Required traits for being able to retrieve block manager address from registry
impl actix::Supervised for BlockManager {}

impl SystemService for BlockManager{}

////////////////////////////////////////////////////////////////////////////////////////
// ACTOR MESSAGES
////////////////////////////////////////////////////////////////////////////////////////

////////////////////////////////////////////////////////////////////////////////////////
// ACTOR MESSAGE HANDLERS
////////////////////////////////////////////////////////////////////////////////////////
/// Handler for EpochNotification
impl Handler<EpochNotification<Self>> for BlockManager {
    type Result = ();

    fn handle(&mut self, msg: EpochNotification<Self>, ctx: &mut Context<Self>) {
        debug!("EPOCH NOTIFICATION RECEIVED!");

        // Call callback
        (*msg.callback)(self, ctx, msg.starting_epoch);
    }
}

/// Impl of EpochNotifiable trait
impl EpochNotifiable<Self> for BlockManager {}
