use amqprs::{
    callbacks::DefaultConnectionCallback,
    channel::{
        BasicConsumeArguments, BasicPublishArguments, Channel, ConsumerMessage, QueueBindArguments,
        QueueDeclareArguments,
    },
    connection::{Connection, OpenConnectionArguments},
    BasicProperties,
};

use log::{debug, error, info};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{Error, IncomingMessages, MessageQueue, Result, Secret};

#[derive(Debug, Clone, Deserialize)]
pub struct RabbitMQOptions {
    /// The hostname of the RabbitMQ server.
    pub host: String,

    /// The port of the RabbitMQ server.
    pub port: u16,

    /// The username to use when connecting to the RabbitMQ server.
    pub user: String,

    /// The password to use when connecting to the RabbitMQ server.
    pub password: Secret,
}

/// The rabbit mq implementation of the message queue trait.
pub struct RabbitMQ {
    channel: Channel,

    #[allow(dead_code)]
    connection: Connection,
}

impl RabbitMQ {
    /// Creates a new agent with the given options.
    ///
    /// # Arguments
    /// * `options` - The options to use when connecting to the RabbitMQ server.
    /// * `queues` - The list of queues to declare.
    pub async fn new<S, I>(options: &RabbitMQOptions, queues: I) -> Result<Self>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        // try to open a connection to the RabbitMQ server
        info!("Opening connection to RabbitMQ server...");
        info!("{}:{}", options.host, options.port);
        let connection = match Connection::open(&OpenConnectionArguments::new(
            &options.host,
            options.port,
            &options.user,
            options.password.secret(),
        ))
        .await
        {
            Ok(connection) => connection,
            Err(e) => {
                error!("Failed to open connection to RabbitMQ server: {}", e);
                return Err(Error::MessageQueue(Box::new(e)));
            }
        };
        info!("Opening connection to RabbitMQ server...DONE");

        // register the default connection callback
        connection
            .register_callback(DefaultConnectionCallback)
            .await
            .map_err(|e| {
                error!("Failed to register default connection callback: {}", e);
                Error::MessageQueue(Box::new(e))
            })?;

        // open a channel
        info!("Opening channel...");
        let channel = match connection.open_channel(None).await {
            Ok(channel) => channel,
            Err(e) => {
                error!("Failed to open channel: {}", e);
                return Err(Error::MessageQueue(Box::new(e)));
            }
        };
        info!("Opening channel...DONE");

        // declare queues
        for queue in queues {
            info!("Declare queue: {}", queue.as_ref());
            match channel
                .queue_declare(QueueDeclareArguments::durable_client_named(queue.as_ref()))
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to declare queue: {}", e);
                    return Err(Error::MessageQueue(Box::new(e)));
                }
            }
            info!("Declare queue: {}...DONE", queue.as_ref());

            // bind the channel to the queue
            info!("Binding channel to queue: {}", queue.as_ref());
            channel
                .queue_bind(QueueBindArguments::new(
                    queue.as_ref(),
                    "amq.topic",
                    queue.as_ref(),
                ))
                .await
                .map_err(|e| {
                    error!("Failed to bind channel to incoming tasks queue: {}", e);
                    Error::MessageQueue(Box::new(e))
                })?;
        }

        Ok(Self {
            connection,
            channel,
        })
    }
}

impl MessageQueue for RabbitMQ {
    type Queue = RabbitMQQueue;

    async fn publish<M>(&self, queue: &str, message: M) -> Result<()>
    where
        M: Sized + Serialize + Send,
    {
        let args = BasicPublishArguments::new("amq.topic", queue);

        let message =
            serde_json::to_vec(&message).map_err(|e| Error::Serialization(Box::new(e)))?;

        self.channel
            .basic_publish(BasicProperties::default(), message, args)
            .await
            .unwrap();

        Ok(())
    }

    async fn subscribe(&self, queue: &str) -> Result<Self::Queue> {
        let args = BasicConsumeArguments {
            queue: queue.to_string(),
            consumer_tag: String::new(),
            no_local: false,
            no_ack: false,
            exclusive: false,
            no_wait: false,
            arguments: Default::default(),
        };

        let (_, receiver) = self.channel.basic_consume_rx(args).await.map_err(|e| {
            error!(
                "Failed to consume messages from incoming tasks queue: {}",
                e
            );
            Error::MessageQueue(Box::new(e))
        })?;

        let queue = RabbitMQQueue { ch: receiver };

        Ok(queue)
    }
}

/// A single rabbit mq queue.
pub struct RabbitMQQueue {
    ch: UnboundedReceiver<ConsumerMessage>,
}

impl IncomingMessages for RabbitMQQueue {
    async fn recv<M: Send + Sized + DeserializeOwned>(&mut self) -> Option<Result<M>> {
        if let Some(message) = self.ch.recv().await {
            if let Some(content) = message.content {
                debug!(
                    "Message result content: {}",
                    String::from_utf8_lossy(&content)
                );

                // try to deserialize the message
                let de_message = match serde_json::from_reader::<&[u8], _>(content.as_ref()) {
                    Ok(de_message) => de_message,
                    Err(e) => {
                        error!("Failed to parse message result!!!: {}", e);
                        return Some(Err(Error::Serialization(Box::new(e))));
                    }
                };

                Some(Ok(de_message))
            } else {
                error!("Received result message without content");
                Some(Err(Error::MessageQueueNoMessage))
            }
        } else {
            None
        }
    }
}
