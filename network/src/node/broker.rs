//
// MIT License
//
// Copyright (c) 2018 Stegos
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
//!
//! Message broker
//!

use failure::Error;
use fnv::FnvHashMap;
use futures::sync::mpsc;
use futures::Stream;
use futures::{Async, Future, Poll};
use libp2p::floodsub::{self, TopicHash};

#[derive(Clone, Debug)]
enum PubsubMessage {
    Subscribe {
        topic: String,
        handler: mpsc::UnboundedSender<Vec<u8>>,
    },
    Publish {
        topic: String,
        data: Vec<u8>,
    },
}

/// Manages subscriptions to topics
///
#[derive(Clone, Debug)]
pub struct BrokerHandler {
    upstream: mpsc::UnboundedSender<PubsubMessage>,
}

impl BrokerHandler {
    /// Subscribe to topic, returns Stream<Vec<u8>> of messages incoming to topic
    pub fn subscribe<S>(&self, topic: &S) -> Result<mpsc::UnboundedReceiver<Vec<u8>>, Error>
    where
        S: Into<String> + Clone,
    {
        let topic: String = topic.clone().into();
        debug!("net: *Subscribed to topic '{}'*", &topic);
        let (tx, rx) = mpsc::unbounded();
        let msg = PubsubMessage::Subscribe { topic, handler: tx };
        self.upstream.unbounded_send(msg)?;
        Ok(rx)
    }
    /// Published message to topic
    pub fn publish<S>(&self, topic: &S, data: Vec<u8>) -> Result<(), Error>
    where
        S: Into<String> + Clone,
    {
        let topic: String = topic.clone().into();
        debug!("net: *Publishing message to topic '{}'*", &topic);
        let msg = PubsubMessage::Publish {
            topic: topic.clone().into(),
            data,
        };
        self.upstream.unbounded_send(msg)?;
        Ok(())
    }
}

enum Message {
    Pubsub(PubsubMessage),
    Input(floodsub::Message),
}

pub struct Broker {
    consumers: FnvHashMap<TopicHash, Vec<mpsc::UnboundedSender<Vec<u8>>>>,
    pubsub_rx: Box<Stream<Item = Message, Error = ()> + Send>,
    floodsub_ctl: floodsub::FloodSubController,
}

impl Broker {
    pub fn new(
        input: floodsub::FloodSubReceiver,
        floodsub_ctl: floodsub::FloodSubController,
    ) -> (Self, BrokerHandler) {
        let (tx, rx) = mpsc::unbounded();

        let messages =
            rx.map(|m| Message::Pubsub(m))
                .select(input.map(|m| Message::Input(m)).map_err(|e| {
                    error!("Error reading from floodsub receiver: {}", e);
                }));

        let broker = Broker {
            consumers: FnvHashMap::default(),
            // input,
            // downstream: rx,
            pubsub_rx: Box::new(messages),
            floodsub_ctl,
        };

        let broker_handler = BrokerHandler { upstream: tx };
        (broker, broker_handler)
    }
}

impl Future for Broker {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match self.pubsub_rx.poll() {
                Ok(Async::Ready(msg)) => match msg {
                    Some(Message::Pubsub(m)) => match m {
                        PubsubMessage::Subscribe { topic, handler } => {
                            let new_topic = floodsub::TopicBuilder::new(topic).build();
                            let topic_hash = new_topic.hash();
                            self.consumers
                                .entry(topic_hash.clone())
                                .or_insert(vec![])
                                .push(handler);
                            self.floodsub_ctl.subscribe(&new_topic);
                        }
                        PubsubMessage::Publish { topic, data } => {
                            let new_topic = floodsub::TopicBuilder::new(topic).build();
                            self.floodsub_ctl.publish(&new_topic, data);
                        }
                    },
                    Some(Message::Input(m)) => {
                        for t in m.topics.into_iter() {
                            let consumers = self.consumers.entry(t).or_insert(vec![]);
                            for i in 0..consumers.len() {
                                if consumers[i].is_closed() {
                                    consumers.remove(i);
                                } else {
                                    if let Err(e) = consumers[i].unbounded_send(m.data.clone()) {
                                        error!("Error sending data to consumer: {}", e);
                                        consumers.remove(i);
                                    }
                                }
                            }
                        }
                    }
                    None => return Ok(Async::Ready(())), // All streams are done!
                },
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => {
                    error!("Error in Broker Future: {:?}", e);
                    return Err(());
                }
            }
        }
    }
}
