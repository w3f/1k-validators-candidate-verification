#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate prettytable;

use chaindata::ChainData;
use chaindata::StashAccount;
use prettytable::{format, Cell, Row, Table};
use std::convert::TryFrom;
use std::{collections::HashMap, vec};
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
    nominators: Vec<StashAccount<AccountId32>>,
) -> Result<()> {
    let chaindata = ChainData::<DefaultNodeRuntime>::new(chain_data_hostname).await?;
    let candidates = fetch_from_endpoint(candidate_hostname).await?;

    let mut ledger_lookups = chaindata
        .fetch_staking_ledgers_by_stashes(&candidates, None)
        .await?;

    // Only retain those accounts which have a ledger.
    ledger_lookups.retain(|l| {
        if l.last_claimed().is_none() {
            warn!("No ledger was found for {} (name \"{}\"). This occurs when no stake has been bonded.", l.account_str(), l.name().unwrap_or("N/A"));
            false
        } else {
            true
        }
    });

    // Sort based on last claimed Era index.
    ledger_lookups.sort_by(|a, b| {
        // Unwrapping is fine since this cases has been handled in the retain mechanism above.
        b.last_claimed()
            .unwrap()
            .unwrap_or(0)
            .partial_cmp(&a.last_claimed().unwrap().unwrap_or(0))
            .unwrap()
    });

    // Fetch nominations.
    let mut nominations = vec![];
    for nominator in &nominators {
        nominations.append(
            &mut chaindata
                .fetch_nominations_by_stash(nominator, None)
                .await?,
        );
    }

    // Display table of candidates.
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(row![
        "Stash",
        "Name",
        "Last claimed (Era)",
        "Nominated by targets"
    ]);

    for lookup in ledger_lookups {
        let address = lookup.account_str();
        let name = lookup.name().unwrap_or("N/A");
        let last_claimed = lookup
            .last_claimed()
            // Unwrapping is fine since this cases has been handled in the
            // retain mechanism above.
            .unwrap()
            .map(|era| era.to_string())
            .unwrap_or("N/A".to_string());

        let is_nominated = if nominations.contains(lookup.account()) {
            "YES"
        } else {
            "no"
        };

        table.add_row(row![address, name, last_claimed, is_nominated]);
    }

    table.printstd();

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
        vec![
            StashAccount::<AccountId32>::try_from(
                "EX9uchmfeSqKTM7cMMg8DkH49XV8i4R7a7rqCn8btpZBHDP",
            )
            .unwrap(),
            StashAccount::<AccountId32>::try_from(
                "G1rrUNQSk7CjjEmLSGcpNu72tVtyzbWdUvgmSer9eBitXWf",
            )
            .unwrap(),
            StashAccount::<AccountId32>::try_from(
                "HgTtJusFEn2gmMmB5wmJDnMRXKD6dzqCpNR7a99kkQ7BNvX",
            )
            .unwrap(),
        ],
    )
    .await
    .unwrap();
}
