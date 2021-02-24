use crate::Result;
use substrate_subxt::staking::{LedgerStoreExt, Staking, StakingLedger};
use substrate_subxt::{Client, ClientBuilder, DefaultNodeRuntime, Runtime};

pub struct ChainData<R: Runtime> {
    client: Client<R>,
}

impl<R: Runtime + Staking> ChainData<R> {
    async fn new(hostname: &str) -> Result<ChainData<R>> {
        Ok(ChainData {
            client: ClientBuilder::<R>::new().set_url(hostname).build().await?,
        })
    }
    async fn fetch_staking_ledger(
        &self,
        account: R::AccountId,
        at: Option<R::Hash>,
    ) -> Result<Option<StakingLedger<R::AccountId, R::Balance>>> {
        self.client
            .ledger(account, at)
            .await
            .map_err(|err| err.into())
    }
}
