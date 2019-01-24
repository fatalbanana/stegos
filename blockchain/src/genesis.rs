//! Genesis Block.

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

use crate::block::*;
use crate::output::*;
use chrono::prelude::Utc;
use std::collections::BTreeSet;
use stegos_crypto::hash::Hash;
use stegos_crypto::pbc::secure as cosi_keys;
use stegos_keychain::KeyChain;

/// Genesis blocks.
pub fn genesis(keychains: &[KeyChain], stake: i64, coins: i64) -> Vec<Block> {
    let mut blocks = Vec::with_capacity(2);

    // Both block are created at the same time in the same epoch.
    let version: u64 = 1;
    let epoch: u64 = 1;
    let timestamp = Utc::now().timestamp() as u64;

    //
    // Create initial Monetary Block.
    //
    let block1 = {
        let previous = Hash::digest(&"genesis".to_string());
        let base = BaseBlockHeader::new(version, previous, epoch, timestamp);

        //
        // Genesis doesn't have inputs
        //
        let inputs = Vec::<Hash>::new();

        //
        // Genesis has one PaymentOutput + N * EscrowOutput, where N is the number of validators.
        // Node #1 receives all moneys except stakes.
        // All nodes gets `stake` money staked.
        //
        let mut outputs: Vec<Output> = Vec::with_capacity(1 + keychains.len());

        // Create PaymentOutput for node #1.
        let sender_skey = &keychains[0].wallet_skey;
        let recipient_pkey = &keychains[0].wallet_pkey;
        let mut coins1: i64 = coins - keychains.len() as i64 * stake;
        let (output, gamma) = Output::new_payment(timestamp, sender_skey, recipient_pkey, coins1)
            .expect("genesis has valid public keys");
        outputs.push(output);

        // Create EscrowOutput for each node.
        for keys in keychains {
            let output = Output::new_escrow(
                timestamp,
                &keys.wallet_skey,
                &keys.wallet_pkey,
                &keys.cosi_pkey,
                stake,
            )
            .expect("genesis has valid public keys");
            coins1 += stake;
            outputs.push(output);
        }

        assert_eq!(coins, coins1);
        MonetaryBlock::new(base, gamma, &inputs, &outputs)
    };

    //
    // Create initial Key Block.
    //
    let block2 = {
        let previous = Hash::digest(&block1);
        let base = BaseBlockHeader::new(version, previous, epoch, timestamp);

        let witnesses: BTreeSet<cosi_keys::PublicKey> =
            keychains.iter().map(|p| p.cosi_pkey.clone()).collect();
        let leader = keychains[0].cosi_pkey.clone();

        KeyBlock::new(base, leader, witnesses)
    };

    blocks.push(Block::MonetaryBlock(block1));
    blocks.push(Block::KeyBlock(block2));

    blocks
}
