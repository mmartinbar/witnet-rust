use log::{debug, error, info, warn};
use std::{net::SocketAddr, time::Duration};

use actix::{
    fut::FutureResult, ActorFuture, Addr, AsyncContext, Context, ContextFutureSpawner, Handler,
    MailboxError, Message, System, SystemService, WrapFuture,
};

use crate::actors::{
    connections_manager::{messages::OutboundTcpConnect, ConnectionsManager},
    peers_manager::{
        messages::{GetRandomPeer, PeersSocketAddrResult},
        PeersManager,
    },
    session::{messages::GetPeers, Session},
};

use witnet_p2p::sessions::Sessions;

mod actor;
mod handlers;
/// Messages for sessions manager
pub mod messages;

/// SessionsManager actor
#[derive(Default)]
pub struct SessionsManager {
    // Registered Sessions
    sessions: Sessions<Addr<Session>>,
}

impl SessionsManager {
    /// Method to periodically bootstrap outbound Sessions
    fn bootstrap_peers(&self, ctx: &mut Context<Self>, bootstrap_peers_period: Duration) {
        // Schedule the bootstrap with a given period
        ctx.run_later(bootstrap_peers_period, move |act, ctx| {
            info!(
                "Number of outbound sessions {}",
                act.sessions.get_num_outbound_sessions()
            );
            info!(
                "Number of inbound sessions {}",
                act.sessions.get_num_inbound_sessions()
            );

            // Check if bootstrap is needed
            if act.sessions.is_outbound_bootstrap_needed() {
                // Get peers manager address
                let peers_manager_addr = System::current().registry().get::<PeersManager>();

                // Start chain of actions
                peers_manager_addr
                    // Send GetPeer message to peers manager actor
                    // This returns a Request Future, representing an asynchronous message sending process
                    .send(GetRandomPeer)
                    // Convert a normal future into an ActorFuture
                    .into_actor(act)
                    // Process the response from the peers manager
                    // This returns a FutureResult containing the socket address if present
                    .then(|res, act, _ctx| {
                        // Process the response from peers manager
                        act.process_get_peer_response(res)
                    })
                    // Process the socket address received
                    // This returns a FutureResult containing a success or error
                    .and_then(|address, _act, _ctx| {
                        debug!("Trying to create a new outbound connection to {}", address);

                        // Get ConnectionsManager from registry and send an OutboundTcpConnect message to it
                        let connections_manager_addr =
                            System::current().registry().get::<ConnectionsManager>();
                        connections_manager_addr.do_send(OutboundTcpConnect { address });

                        actix::fut::ok(())
                    })
                    .wait(ctx);
            }

            // Reschedule the bootstrap peers task
            act.bootstrap_peers(ctx, bootstrap_peers_period);
        });
    }

    /// Method to periodically discover peers
    fn discovery_peers(&self, ctx: &mut Context<Self>, discovery_peers_period: Duration) {
        // Schedule the discovery_peers with a given period
        ctx.run_later(discovery_peers_period, move |act, ctx| {
            // Send Anycast(GetPeers) message
            ctx.notify(messages::Anycast {
                command: GetPeers {},
            });
            act.discovery_peers(ctx, discovery_peers_period);
        });
    }

    /// Method to process peers manager GetPeer response
    fn process_get_peer_response(
        &mut self,
        response: Result<PeersSocketAddrResult, MailboxError>,
    ) -> FutureResult<SocketAddr, (), Self> {
        response
            // Unwrap the Result<PeersSocketAddrResult, MailboxError>
            .unwrap_or_else(|_| {
                error!("Unsuccessful communication with peers manager");
                Ok(None)
            })
            // Unwrap the PeersSocketAddrResult
            .unwrap_or_else(|_| {
                error!("An error happened in peers manager when getting a peer");
                None
            })
            // Check if PeersSocketAddrResult returned `None`
            .or_else(|| {
                warn!("No peer obtained from peers manager");
                None
            })
            // Filter the result checking if outbound address is eligible as new peer
            .filter(|address: &SocketAddr| {
                self.sessions.is_outbound_address_eligible(address.clone())
            })
            // Check if there is a peer after filter
            .or_else(|| {
                warn!("No eligible peer obtained from peers manager");
                None
            })
            // Convert Some(SocketAddr) or None to FutureResult<SocketAddr, (), Self>
            .map(actix::fut::ok)
            .unwrap_or_else(|| actix::fut::err(()))
    }

    /// Method to process Session SendMessage response
    fn process_command_response<T>(
        &mut self,
        response: &Result<T::Result, MailboxError>,
    ) -> FutureResult<(), (), Self>
    where
        T: Message,
        Session: Handler<T>,
    {
        match response {
            Ok(_) => actix::fut::ok(()),
            Err(_) => actix::fut::err(()),
        }
    }
}

/// Required traits for being able to retrieve SessionsManager address from registry
impl actix::Supervised for SessionsManager {}

impl SystemService for SessionsManager {}
