mod rabbit_mq;

use serde::{de::DeserializeOwned, Serialize};

use crate::Result;

use std::future::Future;

pub use rabbit_mq::*;

/// An asynchronous message queue that can be used to receive messages.
pub trait IncomingMessages: Send + Sync {
    /// Receives a message from the message queue. The function blocks until a message is received.
    /// If the message queue is closed, it will return `None`.
    fn recv<M: Send + Sized + DeserializeOwned>(
        &mut self,
    ) -> impl Future<Output = Option<Result<M>>> + Send;
}

/// The trait for the message queue backend.
pub trait MessageQueue: Send + Sync {
    type Queue: IncomingMessages;

    /// Publishes a new message to the specified message queue.
    ///
    /// # Arguments
    /// * `queue` - The message queue to which we publish the message.
    /// * `message` - The task to publish.
    fn publish<M>(&self, queue: &str, message: M) -> impl Future<Output = Result<()>> + Send
    where
        M: Sized + Serialize + Send;

    /// Subscribes to a message queue and returns a future that resolves to a message queue.
    ///
    /// # Arguments
    /// * `queue` - The message queue to which we subscribe.
    fn subscribe(&self, queue: &str) -> impl Future<Output = Result<Self::Queue>> + Send;
}
