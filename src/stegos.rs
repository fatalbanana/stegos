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

mod console;
mod consts;

#[macro_use]
extern crate log;
extern crate log4rs;
#[macro_use]
extern crate clap;
extern crate dirs;
extern crate failure;
extern crate futures;
extern crate libp2p;
extern crate stegos_blockchain;
extern crate stegos_config;
extern crate stegos_crypto;
extern crate stegos_keychain;
extern crate stegos_network;
extern crate stegos_randhound;
extern crate tokio;
extern crate tokio_stdin;
extern crate tokio_timer;

use clap::{App, Arg, ArgMatches};
use console::*;
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Config as LogConfig, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::{Error as LogError, Handle as LogHandle};
use std::error::Error;
use std::path::PathBuf;
use std::process;
use stegos_blockchain::Blockchain;
use stegos_config::{Config, ConfigError};
use stegos_keychain::*;
use stegos_network::Node;
use stegos_randhound::*;

fn load_configuration(args: &ArgMatches) -> Result<Config, Box<Error>> {
    if let Some(cfg_path) = args.value_of_os("config") {
        // Use --config argument for configuration.
        return Ok(stegos_config::from_file(cfg_path)?);
    }

    // Use ~/.config/stegos.toml for configuration.
    let cfg_path = dirs::config_dir()
        .unwrap_or(PathBuf::from(r"."))
        .join(PathBuf::from(consts::CONFIG_FILE_NAME));
    match stegos_config::from_file(cfg_path) {
        Ok(cfg) => return Ok(cfg),
        Err(e) => {
            match e {
                // Don't raise an error on missing configuration file.
                ConfigError::NotFoundError => Ok(Default::default()),
                _ => return Err(Box::new(e)),
            }
        }
    }
}

fn initialize_logger(cfg: &Config) -> Result<LogHandle, LogError> {
    // Try to load log4rs config file
    let handle = match log4rs::load_config_file(
        PathBuf::from(&cfg.general.log4rs_config),
        Default::default(),
    ) {
        Ok(config) => log4rs::init_config(config)?,
        Err(e) => {
            error!("Failed to read log4rs config file: {}", e);
            println!("Failed to read log4rs config file: {}", e);
            let stdout = ConsoleAppender::builder()
                .encoder(Box::new(PatternEncoder::new(
                    "{d(%Y-%m-%d %H:%M:%S)(local)} [{t}] {h({l})} {M}: {m}{n}",
                ))).build();
            let config = LogConfig::builder()
                .appender(Appender::builder().build("stdout", Box::new(stdout)))
                .logger(Logger::builder().build("stegos_network", LevelFilter::Debug))
                .build(Root::builder().appender("stdout").build(LevelFilter::Info))
                .expect("console logger should never fail");
            log4rs::init_config(config)?
        }
    };
    Ok(handle)
}

fn run() -> Result<(), Box<Error>> {
    let args = App::new("Stegos")
        .version(crate_version!())
        .author("Stegos AG <info@stegos.cc>")
        .about("Stegos is a completely anonymous and confidential cryptocurrency.")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Path to stegos.toml configuration file")
                .takes_value(true),
        ).get_matches();

    // Parse configuration
    let cfg = load_configuration(&args)?;

    // Initialize logger
    initialize_logger(&cfg)?;

    // Initialize keychain
    let keychain = KeyChain::new(&cfg.keychain)?;

    // Initialize blockchain
    let mut _blockchain = Blockchain::new();

    // Initialize network
    let mut rt = tokio::runtime::current_thread::Runtime::new()?;
    let my_id = cfg.network.node_id.clone();
    let node = Node::new(&cfg.network, &keychain);
    let (node_future, broker) = node.run()?;

    // Initialize console service
    let console_service = ConsoleService::new(node.clone(), broker.clone());
    rt.spawn(console_service);

    // Initialize randhound
    let randhound = RandHoundService::new(broker.clone(), &my_id)?;
    rt.spawn(randhound);

    // Start main event loop
    rt.block_on(node_future)
        .expect("errors are handled earlier");

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        error!("{}", e);
        process::exit(1)
    };
}

#[cfg(test)]
mod tests {
    extern crate simple_logger;

    #[test]
    fn log_test() {
        simple_logger::init_with_level(log::Level::Debug).unwrap_or_default();

        trace!("This is trace output");
        debug!("This is debug output");
        info!("This is info output");
        warn!("This is warn output");
        error!("This is error output");
        assert_eq!(2 + 2, 4);
    }
}
