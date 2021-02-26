#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate prettytable;

use chaindata::ChainData;
pub use chaindata::StashAccount;
use prettytable::{format, Table};
use std::convert::TryFrom;
use substrate_subxt::sp_core::crypto::AccountId32;
use substrate_subxt::{DefaultNodeRuntime, Runtime};

mod chaindata;

pub type Result<T> = std::result::Result<T, anyhow::Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    pub networks: Vec<NetworkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NetworkConfig {
    pub network: Network,
    pub candidate_hostname: String,
    pub chaindata_hostname: String,
    pub watch_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    Polkadot,
    Kusama,
}

pub async fn generate_candidate_report<R: Runtime>(
    candidate_hostname: &str,
    chaindata_hostname: &str,
    nominators: Vec<StashAccount<AccountId32>>,
) -> Result<()> {
    let chaindata = ChainData::<DefaultNodeRuntime>::new(chaindata_hostname).await?;
    let candidates = fetch_from_endpoint(candidate_hostname).await?;

    debug!("Fetching ledgers of candidates");
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
    debug!("Fetching nominations of target addresses");
    let mut nominations = vec![];
    for nominator in &nominators {
        nominations.append(
            &mut chaindata
                .fetch_nominations_by_stash(nominator, None)
                .await?,
        );
    }

    // Calculate the average Era where rewards were last claimed.
    let avg_last_claimed: u32 = ledger_lookups
        .iter()
        // Unwrapping is fine since this cases has been handled in the retain mechanism above.
        .map(|lookup| lookup.last_claimed().unwrap().unwrap_or(0))
        .sum::<u32>()
        / ledger_lookups.iter().count() as u32;

    // Display table of candidates.
    info!("Generating report table");
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
            .unwrap_or(0);

        let is_nominated = if nominations
            .iter()
            .find(|target| target.stash() == lookup.account().stash())
            .is_some()
        {
            "YES"
        } else {
            "no"
        };

        table.add_row(row![
            address,
            name,
            {
                if last_claimed == 0 {
                    "N/A".to_string()
                } else {
                    last_claimed.to_string()
                }
            },
            is_nominated
        ]);

        if last_claimed < avg_last_claimed {
            warn!(
                "Validator {} (name \"{}\") lags behind in claiming rewards (last claim in Era {})",
                address, name, last_claimed
            );
        }
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
#[serde(rename_all = "snake_case")]
pub struct Candidate {
    pub name: String,
    pub stash: String,
}
