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
    let candidates = CandidateEndpoint::fetch_from_endpoint(candidate_hostname).await?;

    println!("Fetching data");
    let mut ledgers = chaindata
        .fetch_staking_ledgers_by_stashes(&candidates.list_stashes(), None)
        .await?;

    ledgers.sort_by(|a, b| {
        b.claimed_rewards
            .last()
            .unwrap_or(&0)
            .partial_cmp(a.claimed_rewards.last().unwrap_or(&0))
            .unwrap()
    });

    println!("Stash,Name,Last claimed (Era)");
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
        "wss://rpc.polkadot.io",
        "https://polkadot.w3f.community/candidates",
    )
    .await
    .unwrap();
}
