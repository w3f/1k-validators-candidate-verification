use crate::Result;
use std::{convert::TryFrom, vec};
use substrate_subxt::sp_core::crypto::{AccountId32, Ss58Codec};
use substrate_subxt::staking::{
    BondedStoreExt, LedgerStoreExt, NominatorsStoreExt, Staking, StakingLedger,
};
use substrate_subxt::system::System;
use substrate_subxt::{Client, ClientBuilder, DefaultNodeRuntime, Runtime};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct StashAccount<T>(T);

impl<T> StashAccount<T> {
    fn raw(&self) -> &T {
        &self.0
    }
}

impl From<AccountId32> for StashAccount<AccountId32> {
    fn from(val: AccountId32) -> Self {
        StashAccount(val)
    }
}

impl<T: Ss58Codec> TryFrom<String> for StashAccount<T> {
    type Error = anyhow::Error;

    fn try_from(val: String) -> Result<Self> {
        Ok(<Self as TryFrom<&str>>::try_from(&val)?)
    }
}

impl<'a, T: Ss58Codec> TryFrom<&'a str> for StashAccount<T> {
    type Error = anyhow::Error;

    fn try_from(val: &'a str) -> Result<Self> {
        Ok(StashAccount(
            T::from_ss58check_with_version(val)
                .map_err(|err| {
                    anyhow!(
                        "failed to convert value to Runtime account address: {:?}",
                        err
                    )
                })?
                .0,
        ))
    }
}

pub struct NominatedAccount<T>(T);

impl<T> NominatedAccount<T> {
    fn raw(&self) -> &T {
        &self.0
    }
}

pub struct ChainData<R: Runtime> {
    client: Client<R>,
}

impl<R: Runtime + Staking> ChainData<R> {
    pub async fn new(hostname: &str) -> Result<ChainData<R>> {
        Ok(ChainData {
            client: ClientBuilder::<R>::new().set_url(hostname).build().await?,
        })
    }
    pub async fn fetch_staking_ledgers_by_stashes(
        &self,
        stashes: &[&StashAccount<R::AccountId>],
        at: Option<R::Hash>,
    ) -> Result<Vec<StakingLedger<R::AccountId, R::Balance>>> {
        let mut ledgers = vec![];

        for stash in stashes {
            let controller = self.client.bonded(stash.raw().clone(), at).await?;

            if controller.is_none() {
                continue;
            }

            if let Some(ledger) = self.client.ledger(controller.unwrap(), at).await? {
                ledgers.push(ledger);
            }
        }

        Ok(ledgers)
    }
    pub async fn fetch_nominations_by_stash(
        &self,
        stash: &StashAccount<R::AccountId>,
        at: Option<R::Hash>,
    ) -> Result<Vec<NominatedAccount<R::AccountId>>> {
        self.client
            .nominators(stash.raw().clone(), at)
            .await
            .map(|n| {
                n.map(|nominations| {
                    nominations
                        .targets
                        .into_iter()
                        .map(|t| NominatedAccount(t))
                        .collect()
                })
                .unwrap_or(vec![])
            })
            .map_err(|err| err.into())
    }
}

#[tokio::test]
async fn fetch_staking_ledger() {
    use std::convert::TryFrom;
    use substrate_subxt::KusamaRuntime;

    let targets = [
        &StashAccount::<AccountId32>::try_from("EX9uchmfeSqKTM7cMMg8DkH49XV8i4R7a7rqCn8btpZBHDP")
            .unwrap(),
        &StashAccount::<AccountId32>::try_from("G1rrUNQSk7CjjEmLSGcpNu72tVtyzbWdUvgmSer9eBitXWf")
            .unwrap(),
        &StashAccount::<AccountId32>::try_from("HgTtJusFEn2gmMmB5wmJDnMRXKD6dzqCpNR7a99kkQ7BNvX")
            .unwrap(),
    ];

    let onchain = ChainData::<KusamaRuntime>::new("wss://kusama-rpc.polkadot.io")
        .await
        .unwrap();

    let ledgers = onchain
        .fetch_staking_ledgers_by_stashes(&targets, None)
        .await
        .unwrap();

    for ledger in ledgers {
        println!("\n\n>> {:?}", ledger);
    }
}
