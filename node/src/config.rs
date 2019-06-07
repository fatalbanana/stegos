//
// MIT License
//
// Copyright (c) 2019 Stegos AG
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

use serde_derive::{Deserialize, Serialize};
use std::time::Duration;

/// Node configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct NodeConfig {
    /// How long wait for transactions before starting to create a new block.
    pub tx_wait_timeout: Duration,
    /// How long wait for micro blocks.
    pub micro_block_timeout: Duration,
    /// How long wait for the keu blocks.
    pub macro_block_timeout: Duration,
    /// The maximal number of inputs + outputs in a transaction.
    pub max_utxo_in_tx: usize,
    /// The maximal number of inputs + outputs in a micro block.
    pub max_utxo_in_block: usize,
    /// The maximal number of inputs + outputs in mempool.
    pub max_utxo_in_mempool: usize,
    /// Loader will send maximum N epoch at time.
    pub loader_speed_in_epoch: u64,
    /// Fixed fee for payment transactions.
    pub payment_fee: i64,
    /// Fixed fee for the stake transactions.
    pub stake_fee: i64,
}

impl Default for NodeConfig {
    fn default() -> Self {
        NodeConfig {
            tx_wait_timeout: Duration::from_secs(10),
            micro_block_timeout: Duration::from_secs(30),
            macro_block_timeout: Duration::from_secs(30),
            max_utxo_in_tx: 10,
            max_utxo_in_block: 1000,
            max_utxo_in_mempool: 10000,
            loader_speed_in_epoch: 100,
            payment_fee: 1_000, // 0.001 STG
            stake_fee: 0,       // free
        }
    }
}
