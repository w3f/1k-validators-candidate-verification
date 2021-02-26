#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate serde;

use chaindata::ChainData;
use chaindata::StashAccount;
use std::collections::HashMap;
use std::convert::TryFrom;
use substrate_subxt::sp_core::crypto::{AccountId32, Ss58AddressFormat, Ss58Codec};
use substrate_subxt::{DefaultNodeRuntime, KusamaRuntime, Runtime};

mod chaindata;

type Result<T> = std::result::Result<T, anyhow::Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    candidate_endpoints: EndpointConfig,
    chain_data_hostname: String,
    watch_stashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointConfig {
    network: Network,
    hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Network {
    Polkadot,
    Kusama,
}

async fn run_candidate_check<R: Runtime>(
    chain_data_hostname: &str,
    candidate_hostname: &str,
) -> Result<()> {
    let chaindata = ChainData::<DefaultNodeRuntime>::new(chain_data_hostname).await?;
    let candidates = fetch_from_endpoint(candidate_hostname).await?;

    println!("Fetching data");
    let ledgers = chaindata
        .fetch_staking_ledgers_by_stashes(&candidates, None)
        .await?;

    println!("Stash,Name,Last claimed (Era)");
    /*
    for ledger in ledgers {
        println!(
            "{},{},{}",
            ledger
                .stash
                .to_ss58check_with_version(Ss58AddressFormat::PolkadotAccount),
            candidates
                .get_name(&StashAccount::from(ledger.stash.clone()))
                .unwrap_or("N/A"),
            ledger.claimed_rewards.last().unwrap_or(&0)
        );
    }
    */

    Ok(())
}

async fn fetch_from_endpoint(endpoint: &str) -> Result<Vec<StashAccount<AccountId32>>> {
    Ok(reqwest::get(endpoint)
        .await?
        .json::<Vec<Candidate>>()
        .await?
        .into_iter()
        .map(|candidate| {
            let (address, name) = (candidate.stash, candidate.name);
            if let Ok(mut account) = StashAccount::try_from(address) {
                account.set_name(name);
                Ok(account)
            } else {
                Err(anyhow!(
                    "failed to convert candidate address to stash account"
                ))
            }
        })
        .collect::<Result<Vec<StashAccount<AccountId32>>>>()?)
}

/// Structure to parse the `/candidates` endpoint. Only required fields are
/// specified.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub name: String,
    pub stash: String,
}

#[tokio::test]
async fn test_run_candidate_check() {
    run_candidate_check::<DefaultNodeRuntime>(
        "wss://rpc.polkadot.io",
        "https://polkadot.w3f.community/candidates",
    )
    .await
    .unwrap();
}
