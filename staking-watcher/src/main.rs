#[macro_use]
extern crate log;

use lib::{generate_candidate_report, Config, Network, Result, StashAccount};
use std::{convert::TryFrom, fs::read_to_string};
use substrate_subxt::sp_core::crypto::AccountId32;
use substrate_subxt::{DefaultNodeRuntime, KusamaRuntime};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Parsing config");
    let config: Config = serde_yaml::from_str(&read_to_string("config/config.yml")?)?;

    for net_conf in config.networks {
        // Convert targets to appropriate type.
        let watch_targets = net_conf
            .watch_targets
            .iter()
            .map(|target| StashAccount::<AccountId32>::try_from(target.as_str()))
            .collect::<Result<Vec<StashAccount<AccountId32>>>>()?;

        // Execute the process.
        match net_conf.network {
            Network::Polkadot => {
                info!("Starting Polkadot candidate report");
                generate_candidate_report::<DefaultNodeRuntime>(
                    &net_conf.candidate_hostname,
                    &net_conf.chaindata_hostname,
                    watch_targets.clone(),
                )
                .await?;
            }
            Network::Kusama => {
                info!("Starting Kusama candidate report");
                generate_candidate_report::<KusamaRuntime>(
                    &net_conf.candidate_hostname,
                    &net_conf.chaindata_hostname,
                    watch_targets.clone(),
                )
                .await?;
            }
        }
    }

    info!("Exiting");

    Ok(())
}
