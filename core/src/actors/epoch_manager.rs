use log::{debug, warn};

use actix::{Actor, Addr, AsyncContext, Context, Handler, Message, SystemService};

//use std::collections::HashMap;

use crate::actors::config_manager::send_get_config_request;

use witnet_config::config::Config;

use witnet_util::timestamp::get_timestamp;

/// Epoch
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Epoch(pub u64);

/// Posible errors when getting the current epoch
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EpochManagerError {
    /// Epoch zero time is unknown
    UnknownEpochZero,
    /// Checkpoint period is unknown
    UnknownCheckpointPeriod,
    // Current time is unknown
    // (unused because get_timestamp() cannot fail)
    //UnknownTimestamp,
    /// Epoch zero is in the future
    EpochZeroInTheFuture,
    /// Overflow when calculating the epoch timestamp
    Overflow,
}

////////////////////////////////////////////////////////////////////////////////////////
// ACTOR MESSAGES
////////////////////////////////////////////////////////////////////////////////////////
/// Returns the current epoch
pub struct GetEpoch;

/// Epoch result
pub type EpochResult<T> = Result<T, EpochManagerError>;

impl Message for GetEpoch {
    type Result = EpochResult<Epoch>;
}

/// Subscribe to a single epoch
#[derive(Message)]
pub struct Subscribe<T>
    where
        T: actix::Actor,
        T::Context: actix::AsyncContext<T>,
        T: crate::actors::epoch_manager::EpochNotifiable<T> {
    /// Actor address
    pub address: Addr<T>,

    /// Message to be sent back
    pub message: EpochNotification<T>,
}

////////////////////////////////////////////////////////////////////////////////////////
// ACTOR BASIC STRUCTURE
////////////////////////////////////////////////////////////////////////////////////////
/// Epoch manager actor
#[derive(Debug, Default)]
pub struct EpochManager {
    /// Epoch zero timestamp
    epoch_zero_timestamp: Option<i64>,

    /// Checkpoint period (in seconds)
    checkpoint_period_seconds: Option<u64>,

    //messages: HashMap<u32, EpochNotification<T>>,
    //messages: HashMap<u32, u32>,
    //message: EpochNotification<T>
}

/// Make actor from `ConnectionsManager`
impl Actor for EpochManager {
    /// Every actor has to provide execution `Context` in which it can run.
    type Context = Context<Self>;

    /// Method to be executed when the actor is started
    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Epoch Manager actor has been started!");

        send_get_config_request(self, ctx, Self::process_config)
    }
}

/// Required trait for being able to retrieve connections manager address from system registry
impl actix::Supervised for EpochManager {}

/// Required trait for being able to retrieve connections manager address from system registry
impl SystemService for EpochManager {}

/// Auxiliary methods for `EpochManager` actor
impl EpochManager {
    /// Set the timestamp for the epoch zero
    pub fn set_epoch_zero(&mut self, timestamp: i64) {
        self.epoch_zero_timestamp = Some(timestamp);
    }
    /// Set the checkpoint period between epochs
    pub fn set_period(&mut self, mut period: u64) {
        if period == 0 {
            warn!("Setting the checkpoint period to the minimum value of 1 second");
            period = 1;
        }
        self.checkpoint_period_seconds = Some(period);
    }
    /// Calculate the epoch at the supplied timestamp
    pub fn epoch_at(&self, timestamp: i64) -> EpochResult<Epoch> {
        match (self.epoch_zero_timestamp, self.checkpoint_period_seconds) {
            (Some(zero), Some(period)) => {
                let elapsed = timestamp - zero;
                if elapsed < 0 {
                    Err(EpochManagerError::EpochZeroInTheFuture)
                } else {
                    let epoch = elapsed as u64 / period;
                    Ok(Epoch(epoch))
                }
            }
            (None, _) => Err(EpochManagerError::UnknownEpochZero),
            (_, None) => Err(EpochManagerError::UnknownCheckpointPeriod),
        }
    }
    /// Calculate the current epoch
    pub fn current_epoch(&self) -> EpochResult<Epoch> {
        let now = get_timestamp();
        self.epoch_at(now)
    }
    /// Calculate the timestamp at the start of an epoch
    pub fn epoch_timestamp(&self, epoch: Epoch) -> EpochResult<i64> {
        match (self.epoch_zero_timestamp, self.checkpoint_period_seconds) {
            // Calculate (period * epoch + zero) with overflow checks
            (Some(zero), Some(period)) => period
                .checked_mul(epoch.0)
                .filter(|&x| x <= i64::max_value() as u64)
                .map(|x| x as i64)
                .and_then(|x| x.checked_add(zero))
                .ok_or(EpochManagerError::Overflow),
            (None, _) => Err(EpochManagerError::UnknownEpochZero),
            (_, None) => Err(EpochManagerError::UnknownCheckpointPeriod),
        }
    }
    /// Method to process the configuration received from the config manager
    fn process_config(&mut self, _ctx: &mut <Self as Actor>::Context, config: &Config) {
        self.set_epoch_zero(config.protocol.epoch_zero_timestamp);
        self.set_period(config.protocol.checkpoint_period);
        debug!(
            "Epoch zero timestamp: {}",
            self.epoch_zero_timestamp.unwrap()
        );
    }
}

////////////////////////////////////////////////////////////////////////////////////////
// ACTOR MESSAGE HANDLERS
////////////////////////////////////////////////////////////////////////////////////////
impl Handler<GetEpoch> for EpochManager {
    /// Response for message, which is defined by `ResponseType` trait
    type Result = EpochResult<Epoch>;

    /// Method to handle the InboundTcpConnect message
    fn handle(&mut self, _msg: GetEpoch, _ctx: &mut Self::Context) -> EpochResult<Epoch> {
        let r = self.current_epoch();
        debug!("Current epoch: {:?}", r);
        r
    }
}

impl<T> Handler<Subscribe<T>> for EpochManager
    where
    T: actix::Actor,
    T::Context: actix::AsyncContext<T>,
    T: crate::actors::epoch_manager::EpochNotifiable<T>
{
    /// Result type
    type Result = ();

    /// Method to handle the Subscribe message
    fn handle(&mut self, _msg: Subscribe<T>, _ctx: &mut Self::Context) {
        debug!("SUBSCRIPTION RECEIVED!");

        // Send message back to the actor
        //msg.address.do_send(msg.message);
    }
}

////////////////////////////////////////////////////////////////////////////////////////
// OTHER ACTOR MESSAGES
////////////////////////////////////////////////////////////////////////////////////////
/// Actor message to indicate that when an epoch starts, a callback function should be called
#[derive(Message)]
pub struct EpochNotification<T>
    where
        T: actix::Actor,
        T::Context: actix::AsyncContext<T>,
        T: crate::actors::epoch_manager::EpochNotifiable<T>
{
    /// Epoch that has just started
    pub starting_epoch: u32,

    /// Payload for the epoch notification
    pub callback: Box<dyn Fn(&mut T, &mut T::Context, u32)>,
}

/// Epoch notifiable trait
pub trait EpochNotifiable<T>: Handler<EpochNotification<T>>
    where
        T: Actor,
        T::Context: AsyncContext<T>,
        T: EpochNotifiable<T>
{}