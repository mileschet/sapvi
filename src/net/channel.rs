use async_std::sync::Mutex;
use futures::io::{ReadHalf, WriteHalf};
use futures::AsyncReadExt;
use log::*;
use smol::{Async, Executor};

use std::net::{SocketAddr, TcpStream};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error;
use crate::net::error::{NetError, NetResult};
use crate::net::message_subscriber::{
    MessageSubscriber, MessageSubscriberPtr, MessageSubscription,
};
use crate::net::messages;
use crate::net::settings::SettingsPtr;
use crate::system::{StoppableTask, StoppableTaskPtr, Subscriber, SubscriberPtr, Subscription};

pub type ChannelPtr = Arc<Channel>;

pub struct Channel {
    reader: Mutex<ReadHalf<Async<TcpStream>>>,
    writer: Mutex<WriteHalf<Async<TcpStream>>>,
    address: SocketAddr,
    message_subscriber: MessageSubscriberPtr,
    stop_subscriber: SubscriberPtr<NetError>,
    receive_task: StoppableTaskPtr,
    stopped: AtomicBool,
    settings: SettingsPtr,
}

impl Channel {
    pub fn new(stream: Async<TcpStream>, address: SocketAddr, settings: SettingsPtr) -> Arc<Self> {
        let (reader, writer) = stream.split();
        let reader = Mutex::new(reader);
        let writer = Mutex::new(writer);
        Arc::new(Self {
            reader,
            writer,
            address,
            message_subscriber: MessageSubscriber::new(),
            stop_subscriber: Subscriber::new(),
            receive_task: StoppableTask::new(),
            stopped: AtomicBool::new(false),
            settings,
        })
    }

    pub fn start(self: Arc<Self>, executor: Arc<Executor<'_>>) {
        debug!(target: "net", "Channel::start() [START, address={}]", self.address());
        let self2 = self.clone();
        self.receive_task.clone().start(
            self.clone().receive_loop(),
            // Ignore stop handler
            |result| self2.handle_stop(result),
            NetError::ServiceStopped,
            executor,
        );
        debug!(target: "net", "Channel::start() [END, address={}]", self.address());
    }

    pub async fn stop(&self) {
        debug!(target: "net", "Channel::stop() [START, address={}]", self.address());
        assert_eq!(self.stopped.load(Ordering::Relaxed), false);
        self.stopped.store(false, Ordering::Relaxed);
        let stop_err = Arc::new(NetError::ChannelStopped);
        self.stop_subscriber.notify(stop_err).await;
        self.receive_task.stop().await;
        debug!(target: "net", "Channel::stop() [END, address={}]", self.address());
    }

    pub async fn subscribe_stop(&self) -> Subscription<NetError> {
        debug!(target: "net",
            "Channel::subscribe_stop() [START, address={}]",
            self.address()
        );
        // TODO: this should check the stopped status
        // Call to receive should return ChannelStopped on newly created sub
        let sub = self.stop_subscriber.clone().subscribe().await;
        debug!(target: "net",
            "Channel::subscribe_stop() [END, address={}]",
            self.address()
        );
        sub
    }

    pub async fn send(self: Arc<Self>, message: messages::Message) -> NetResult<()> {
        let packet_type = message.packet_type();
        debug!(target: "net",
            "Channel::send() [START, pkt_type={:?}, address={}]",
            packet_type,
            self.address()
        );
        if self.stopped.load(Ordering::Relaxed) {
            return Err(NetError::ChannelStopped);
        }

        // Catch failure and stop channel, return a net error
        let result = match messages::send_message(&mut *self.writer.lock().await, message).await {
            Ok(()) => Ok(()),
            Err(err) => {
                error!("Channel send error for [{}]: {}", self.address(), err);
                self.stop().await;
                Err(NetError::ChannelStopped)
            }
        };
        debug!(target: "net",
            "Channel::send() [END, pkt_type={:?}, address={}]",
            packet_type,
            self.address()
        );
        result
    }

    pub async fn subscribe_msg(
        self: Arc<Self>,
        packet_type: messages::PacketType,
    ) -> MessageSubscription {
        debug!(target: "net",
            "Channel::subscribe_msg() [START, pkt_type={:?}, address={}]",
            packet_type,
            self.address()
        );
        let sub = self.message_subscriber.clone().subscribe(packet_type).await;
        debug!(target: "net",
            "Channel::subscribe_msg() [END, pkt_type={:?}, address={}]",
            packet_type,
            self.address()
        );
        sub
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    fn is_eof_error(err: &error::Error) -> bool {
        match err {
            error::Error::Io(io_err) => io_err.kind() == std::io::ErrorKind::UnexpectedEof,
            _ => false,
        }
    }

    async fn receive_loop(self: Arc<Self>) -> NetResult<()> {
        debug!(target: "net",
            "Channel::receive_loop() [START, address={}]",
            self.address()
        );
        let reader = &mut *self.reader.lock().await;

        loop {
            let message_result = messages::receive_message(reader).await;
            let message = match message_result {
                Ok(message) => Arc::new(message),
                Err(err) => {
                    if Self::is_eof_error(&err) {
                        info!("Channel {} disconnected", self.address());
                    } else {
                        error!("Read error on channel: {}", err);
                    }
                    debug!(target: "net",
                        "Channel::receive_loop() stopping channel {}",
                        self.address()
                    );
                    self.stop().await;
                    return Err(NetError::ChannelStopped);
                }
            };

            // Send result to our subscribers
            self.message_subscriber.notify(Ok(message)).await;
        }
    }

    async fn handle_stop(self: Arc<Self>, result: NetResult<()>) {
        debug!(target: "net", "Channel::handle_stop() [START, address={}]", self.address());
        match result {
            Ok(()) => panic!("Channel task should never complete without error status"),
            Err(err) => {
                // Send this error to all channel subscribers
                let result = Err(err);
                self.message_subscriber.notify(result).await;
            }
        }
        debug!(target: "net", "Channel::handle_stop() [END, address={}]", self.address());
    }
}
