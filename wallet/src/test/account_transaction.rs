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

use super::*;
use crate::valueshuffle::message::{Message, VsPayload};
use crate::*;
use assert_matches::assert_matches;
use futures::sync::oneshot::Receiver;
use futures::{Async, Future};
use std::time::Duration;
use stegos_crypto::scc::PublicKey;

const PAYMENT_FEE: i64 = 1_000; // 0.001 STG

#[test]
fn empty_log_at_start() {
    Sandbox::start(Default::default(), |mut s| {
        let mut accounts = genesis_accounts(&mut s);
        s.poll();
        let rx = accounts[0].account.request(AccountRequest::HistoryInfo {
            starting_from: Timestamp::now() - Duration::from_secs(1000),
            limit: 1,
        });
        accounts[0].poll();
        let response = get_request(rx);
        info!("{:?}", response);
        assert_eq!(response, AccountResponse::HistoryInfo { log: vec![] });
        s.filter_unicast(&[stegos_node::CHAIN_LOADER_TOPIC])
    });
}

#[test]
fn create_tx() {
    Sandbox::start(Default::default(), |mut s| {
        let mut accounts = genesis_accounts(&mut s);

        s.poll();
        let recipient = accounts[1].account_service.account_pkey;

        let rx = accounts[0].account.request(AccountRequest::Payment {
            recipient,
            amount: 10,
            payment_fee: PAYMENT_FEE,
            comment: "Test".to_string(),
            locked_timestamp: None,
            with_certificate: false,
        });

        accounts[0].poll();
        let response = get_request(rx);
        info!("{:?}", response);
        let tx_hash = match response {
            AccountResponse::TransactionCreated(tx) => {
                assert!(tx.certificates.is_empty());
                tx.tx_hash
            }
            _ => panic!("Wrong respnse to payment request"),
        };

        s.filter_unicast(&[stegos_node::CHAIN_LOADER_TOPIC]);

        // rebroadcast transaction to each node
        s.broadcast(stegos_node::TX_TOPIC);
        let mut commited_status = accounts[0]
            .account
            .request(AccountRequest::WaitForCommit { tx_hash });

        accounts[0].poll();
        assert_eq!(commited_status.poll(), Ok(Async::NotReady));
        s.wait(s.config.node.tx_wait_timeout);
        s.skip_micro_block();

        accounts[0].poll();

        assert_matches!(
            get_request(commited_status),
            AccountResponse::TransactionCommitted(_)
        );
        let commited_status_post = accounts[0]
            .account
            .request(AccountRequest::WaitForCommit { tx_hash });
        accounts[0].poll();
        assert_matches!(
            get_request(commited_status_post),
            AccountResponse::TransactionCommitted(_)
        );
    });
}

// genesis account should has atleast N* amount of accounts
fn precondition_each_account_has_tokens(
    s: &mut Sandbox,
    amount: i64,
    genesis_account: &mut AccountSandbox,
    accounts: &mut [AccountSandbox],
) {
    for new_account in accounts.iter() {
        let rx = genesis_account.account.request(AccountRequest::Payment {
            recipient: new_account.account_service.account_pkey,
            amount,
            payment_fee: PAYMENT_FEE,
            comment: "Test".to_string(),
            locked_timestamp: None,
            with_certificate: false,
        });
        genesis_account.poll();
        assert_matches!(get_request(rx), AccountResponse::TransactionCreated(_));

        s.filter_unicast(&[stegos_node::CHAIN_LOADER_TOPIC]);
        // rebroadcast transaction to each node
        s.broadcast(stegos_node::TX_TOPIC);
        s.wait(s.config.node.tx_wait_timeout);
        s.skip_micro_block();
        genesis_account.poll();
    }

    for new_account in accounts {
        new_account.poll();
        let rx = new_account.account.request(AccountRequest::BalanceInfo {});
        new_account.poll();

        match get_request(rx) {
            AccountResponse::BalanceInfo { balance } => {
                dbg!((balance, amount));
                assert!(balance >= amount);
            }
            _ => panic!("Wrong respnse to payment request"),
        };
    }
}

fn vs_start(
    recipient: PublicKey,
    amount: i64,
    account: &mut AccountSandbox,
) -> Receiver<AccountResponse> {
    let rx = account.account.request(AccountRequest::SecurePayment {
        recipient,
        amount,
        payment_fee: PAYMENT_FEE,
        comment: "Test".to_string(),
        locked_timestamp: None,
    });

    account.poll();

    let response = get_request(rx);
    info!("{:?}", response);
    let tx_hash = match response {
        AccountResponse::ValueShuffleStarted { session_id } => session_id,
        _ => panic!("Wrong respnse to payment request"),
    };

    let mut commited_status = account
        .account
        .request(AccountRequest::WaitForCommit { tx_hash });

    account.poll();
    assert_eq!(commited_status.poll(), Ok(Async::NotReady));

    commited_status
}

/// 3 nodes send monet to 1 recipient, using vs
#[ignore]
#[test]
fn create_vs_tx() {
    const SEND_TOKENS: i64 = 10;
    // send MINIMAL_TOKEN + FEE
    const MIN_AMOUNT: i64 = SEND_TOKENS + PAYMENT_FEE;
    // set micro_blocks to some big value.
    let config = SandboxConfig {
        chain: ChainConfig {
            micro_blocks_in_epoch: 2000,
            ..Default::default()
        },
        ..Default::default()
    };
    Sandbox::start(config, |mut s| {
        let mut accounts = genesis_accounts(&mut s);

        s.poll();
        let (genesis, rest) = accounts.split_at_mut(1);
        precondition_each_account_has_tokens(&mut s, MIN_AMOUNT, &mut genesis[0], rest);

        let num_nodes = accounts.len() - 1;
        let sleep_since_epoch = s.config.node.tx_wait_timeout * num_nodes as u32;

        let recipient = accounts[3].account_service.account_pkey;

        let mut committed_statuses = Vec::new();
        for i in 0..num_nodes {
            let status = vs_start(recipient, SEND_TOKENS, &mut accounts[i]);
            committed_statuses.push(status);
            accounts[i].poll();
        }
        s.filter_unicast(&[stegos_node::CHAIN_LOADER_TOPIC]);

        debug!("===== JOINING TXPOOL =====");
        s.deliver_unicast(stegos_node::txpool::POOL_JOIN_TOPIC);
        s.poll();

        // this code are done just to work with different timeout configuration.
        {
            // we already sleep for 3 microblocks,
            let sleep_time = if stegos_node::txpool::MESSAGE_TIMEOUT > sleep_since_epoch {
                stegos_node::txpool::MESSAGE_TIMEOUT - sleep_since_epoch
            } else {
                Duration::from_millis(0)
            };

            s.wait(sleep_time);
            // if we sleep enought for microblock, then broadcast microblock.
            if sleep_time >= s.config.node.tx_wait_timeout {
                s.skip_micro_block();
                // if we sleep enought for view_change, then filter_view_change.
                if sleep_time >= s.config.node.micro_block_timeout {
                    s.filter_broadcast(&[stegos_node::VIEW_CHANGE_TOPIC]);
                }
            } else {
                // if not, just poll to triger txpoll_anouncment
                s.poll();
            }
        }
        debug!("===== ANONCING TXPOOL =====");
        s.deliver_unicast(stegos_node::txpool::POOL_ANNOUNCE_TOPIC);

        debug!("===== VS STARTED NEW POOL: Send::SharedKeying =====");
        accounts.iter_mut().for_each(AccountSandbox::poll);

        let mut shared_keying = HashMap::new();
        for account in accounts.iter_mut().take(num_nodes) {
            for _ in 0..num_nodes - 1 {
                // each node send message to rest
                let (msg, peer) = account
                    .network
                    .get_unicast::<valueshuffle::message::DirectMessage>(
                        crate::valueshuffle::VALUE_SHUFFLE_TOPIC,
                    );

                // TODO: extend assertion.
                assert_matches!(msg.message,
                    Message::VsMessage{
                        payload: VsPayload::SharedKeying {
                            ..
                        },
                        ..
                });

                debug!("Adding msg to peer = {}", peer);
                let entry = shared_keying.entry(peer).or_insert(Vec::new());
                entry.push(msg); // msg directed to peer
            }
        }

        for account in accounts.iter_mut().take(num_nodes) {
            debug!(
                "Receiving msg to peer = {}",
                account.account_service.network_pkey
            );
            for msg in shared_keying
                .get(&account.account_service.network_pkey)
                .unwrap()
            {
                account.network.receive_unicast(
                    msg.source.pkey,
                    crate::valueshuffle::VALUE_SHUFFLE_TOPIC,
                    msg.clone(),
                )
            }
        }

        debug!("===== VS Receive shared keying: Send::Commitment =====");
        accounts.iter_mut().for_each(AccountSandbox::poll);

        let mut commitments = HashMap::new();
        for account in accounts.iter_mut().take(num_nodes) {
            for _ in 0..num_nodes - 1 {
                // each node send message to rest
                let (msg, peer) = account
                    .network
                    .get_unicast::<valueshuffle::message::DirectMessage>(
                        crate::valueshuffle::VALUE_SHUFFLE_TOPIC,
                    );

                // TODO: extend assertion.
                assert_matches!(msg.message,
                    Message::VsMessage{
                        payload: VsPayload::Commitment {
                            ..
                        },
                        ..
                });

                let entry = commitments.entry(peer).or_insert(Vec::new());
                entry.push(msg); // msg directed to peer
            }
        }

        for account in accounts.iter_mut().take(num_nodes) {
            for msg in commitments
                .get(&account.account_service.network_pkey)
                .unwrap()
            {
                account.network.receive_unicast(
                    msg.source.pkey,
                    crate::valueshuffle::VALUE_SHUFFLE_TOPIC,
                    msg.clone(),
                )
            }
        }

        debug!("===== VS Receive commitment: produce CloakedVals =====");
        accounts.iter_mut().for_each(AccountSandbox::poll);

        let mut cloaked_vals = HashMap::new();
        for account in accounts.iter_mut().take(num_nodes) {
            for _ in 0..num_nodes - 1 {
                // each node send message to rest
                let (msg, peer) = account
                    .network
                    .get_unicast::<valueshuffle::message::DirectMessage>(
                        crate::valueshuffle::VALUE_SHUFFLE_TOPIC,
                    );

                // TODO: extend assertion.
                assert_matches!(msg.message,
                    Message::VsMessage{
                        payload: VsPayload::CloakedVals {
                            ..
                        },
                        ..
                });

                let entry = cloaked_vals.entry(peer).or_insert(Vec::new());
                entry.push(msg); // msg directed to peer
            }
        }

        for account in accounts.iter_mut().take(num_nodes) {
            for msg in cloaked_vals
                .get(&account.account_service.network_pkey)
                .unwrap()
            {
                account.network.receive_unicast(
                    msg.source.pkey,
                    crate::valueshuffle::VALUE_SHUFFLE_TOPIC,
                    msg.clone(),
                )
            }
        }

        debug!("===== VS Receive CloakedVals: produce signatures =====");
        accounts.iter_mut().for_each(AccountSandbox::poll);

        let mut signatures = HashMap::new();
        for account in accounts.iter_mut().take(num_nodes) {
            for _ in 0..num_nodes - 1 {
                // each node send message to rest
                let (msg, peer) = account
                    .network
                    .get_unicast::<valueshuffle::message::DirectMessage>(
                        crate::valueshuffle::VALUE_SHUFFLE_TOPIC,
                    );

                // TODO: extend assertion.
                assert_matches!(msg.message,
                    Message::VsMessage{
                        payload: VsPayload::Signature {
                            ..
                        },
                        ..
                });

                let entry = signatures.entry(peer).or_insert(Vec::new());
                entry.push(msg); // msg directed to peer
            }
        }

        for account in accounts.iter_mut().take(num_nodes) {
            for msg in signatures
                .get(&account.account_service.network_pkey)
                .unwrap()
            {
                account.network.receive_unicast(
                    msg.source.pkey,
                    crate::valueshuffle::VALUE_SHUFFLE_TOPIC,
                    msg.clone(),
                )
            }
        }
        debug!("===== VS Receive Signatures: produce tx =====");
        accounts.iter_mut().for_each(AccountSandbox::poll);
        // rebroadcast transaction to each node
        debug!("===== BROADCAST VS TRANSACTION =====");
        s.broadcast(stegos_node::TX_TOPIC);

        s.wait(s.config.node.tx_wait_timeout);
        s.skip_micro_block();

        for (account, committed_status) in accounts.iter_mut().zip(committed_statuses) {
            account.poll();
            assert_matches!(
                get_request(committed_status),
                AccountResponse::TransactionCommitted(_)
            );
        }
    });
}