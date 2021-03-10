use crate::database::{CandidateState, EraTracker, TimetableStoreReader};
use crate::jury::{RequirementsConfig, RequirementsJudgement, RequirementsJudgementReport};
use crate::system::Candidate;
use crate::Result;
use std::convert::TryFrom;
use substrate_subxt::balances::Balances;
use substrate_subxt::identity::{Identity, IdentityOfStoreExt};
use substrate_subxt::sp_core::crypto::{AccountId32, Ss58Codec};
use substrate_subxt::staking::{
    BondedStoreExt, CurrentEraStoreExt, LedgerStoreExt, PayeeStoreExt, Staking, ValidatorsStoreExt,
};
use substrate_subxt::{Client, ClientBuilder, Runtime};
use tokio::time::{self, Duration};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NetworkAccount<T>(T);

impl From<AccountId32> for NetworkAccount<AccountId32> {
    fn from(val: AccountId32) -> Self {
        NetworkAccount(val)
    }
}

impl TryFrom<Candidate> for NetworkAccount<AccountId32> {
    type Error = anyhow::Error;

    fn try_from(val: Candidate) -> Result<Self> {
        Ok(NetworkAccount(
            AccountId32::from_ss58check(val.stash_str()).map_err(|_err| {
                anyhow!("Failed to convert presumed SS58 string into a NetworkAccount")
            })?,
        ))
    }
}

#[allow(dead_code)]
pub struct RequirementsProceeding<T: Runtime + Balances> {
    client: Client<T>,
    requirements: RequirementsConfig<T::Balance>,
    store: TimetableStoreReader,
    era_tracker: EraTracker,
}

impl<T: Runtime + Balances + Identity + Staking> RequirementsProceeding<T>
where
    T::AccountId: Ss58Codec,
{
    pub async fn new(
        rpc_hostname: &str,
        requirements: RequirementsConfig<T::Balance>,
        store: TimetableStoreReader,
        era_tracker: EraTracker,
    ) -> Result<Self> {
        Ok(RequirementsProceeding {
            client: ClientBuilder::<T>::new()
                .set_url(rpc_hostname)
                .skip_type_sizes_check()
                .build()
                .await?,
            requirements: requirements,
            store: store,
            era_tracker: era_tracker,
        })
    }
    pub async fn wait_for_era_change(&self) -> Result<()> {
        let current = self
            .client
            .current_era(None)
            .await?
            .ok_or(anyhow!("failed to retrieve Era from on-chain service"))?;

        while !self.era_tracker.is_new_era(current).await? {
            time::sleep(Duration::from_secs(10)).await;
        }

        Ok(())
    }
    pub async fn proceed_requirements(
        &self,
        state: CandidateState,
    ) -> Result<RequirementsJudgementReport> {
        debug!(
            "Starting requirement checking process for {} ('{}')",
            state.candidate.stash_str(),
            state.candidate.node_name().as_str()
        );

        let mut jury =
            RequirementsJudgement::<T>::new(&state, &self.requirements, self.store.clone())?;

        let account_id = state.candidate.to_account_id::<T::AccountId>()?;

        // Requirement: Identity.
        debug!("Checking identity requirement");
        let identity = self.client.identity_of(account_id.clone(), None).await?;
        jury.judge_identity(identity);

        // Requirement: Reward destination.
        debug!("Checking reward destination requirement");
        let desination = self.client.payee(account_id.clone(), None).await?;
        jury.judge_reward_destination(desination);

        // Requirement: Commission.
        debug!("Checking commission requirement");
        let prefs = self.client.validators(account_id.clone(), None).await?;
        jury.judge_commission(prefs.commission);

        // Requirement: Controller set.
        debug!("Checking controller requirement");
        let controller = self.client.bonded(account_id.clone(), None).await?;
        jury.judge_stash_controller_deviation(&controller)?;

        // Requirement: Bonded amount.
        debug!("Checking bonded amount requirement");
        if let Some(controller) = controller {
            let ledger = self.client.ledger(controller, None).await?;
            jury.judge_bonded_amount(ledger);
        } else {
            jury.judge_bonded_amount(None);
        }

        // Requirement: Node uptime.
        debug!("Checking node uptime");
        jury.judge_node_uptime(&state.candidate).await?;

        // Requirement: Check client version

        Ok(jury.generate_report())
    }
}
