#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate serde;

use chaindata::ChainData;
use chaindata::StashAccount;
use std::collections::HashMap;
use std::convert::TryFrom;
use substrate_subxt::{sp_core::crypto::AccountId32, sp_runtime::print};
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
    let candidates = CandidateEndpoint::fetch_from_endpoint(candidate_hostname).await?;

    let ledgers = chaindata
        .fetch_staking_ledgers_by_stashes(&candidates.list_stashes(), None)
        .await?;
    for ledger in ledgers {
        println!("\n");
        println!("Stash: {}", ledger.stash);
        println!(
            "Last claimed reward (Era): {}",
            ledger.claimed_rewards.last().unwrap_or(&0)
        );
    }

    Ok(())
}

struct CandidateEndpoint {
    candidates: HashMap<StashAccount<AccountId32>, String>,
}

impl CandidateEndpoint {
    async fn fetch_from_endpoint(endpoint: &str) -> Result<Self> {
        Ok(CandidateEndpoint {
            candidates: {
                let candidates = reqwest::get(endpoint)
                    .await?
                    .json::<Vec<Candidate>>()
                    .await?;

                let mut map = HashMap::new();
                for candidate in candidates {
                    map.insert(StashAccount::try_from(candidate.stash)?, candidate.name);
                }

                map
            },
        })
    }
    fn list_stashes(&self) -> Vec<&StashAccount<AccountId32>> {
        self.candidates.keys().collect()
    }
    fn get_name(&self, stash: &StashAccount<AccountId32>) -> Option<&str> {
        self.candidates.get(stash).map(|s| s.as_str())
    }
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
        "rpc.polkadot.parity",
        "https://polkadot.w3f.community/candidates",
    )
    .await
    .unwrap();
}
