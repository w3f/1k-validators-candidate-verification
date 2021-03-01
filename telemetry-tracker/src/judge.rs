use crate::jury::{RequirementsConfig, RequirementsJudgement, RequirementsJudgementReport};
use crate::Result;
use substrate_subxt::balances::Balances;
use substrate_subxt::identity::{Identity, IdentityOfStoreExt};
use substrate_subxt::staking::{
    BondedStoreExt, LedgerStoreExt, PayeeStoreExt, Staking, ValidatorsStoreExt,
};
use substrate_subxt::{Client, ClientBuilder, Runtime};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Candidate<T>(T);

impl<T: Clone> Candidate<T> {
    pub fn raw(&self) -> &T {
        &self.0
    }
    pub fn to_stash(&self) -> T {
        self.0.clone()
    }
}

pub struct RequirementsProceeding<T: Runtime> {
    client: Client<T>,
}

impl<T: Runtime + Balances + Identity + Staking> RequirementsProceeding<T> {
    pub async fn new(rpc_hostname: &str) -> Result<Self> {
        Ok(RequirementsProceeding {
            client: ClientBuilder::<T>::new()
                .set_url(rpc_hostname)
                .build()
                .await?,
        })
    }
    pub async fn proceed_requirements(
        &self,
        candidate: Candidate<T::AccountId>,
        requirements: RequirementsConfig<T::Balance>,
    ) -> Result<RequirementsJudgementReport<T::AccountId>> {
        let mut jury = RequirementsJudgement::<T>::new(candidate.clone(), requirements);

        // Requirement: Identity.
        let identity = self.client.identity_of(candidate.to_stash(), None).await?;
        jury.judge_identity(identity);

        // Requirement: Reward destination.
        let desination = self.client.payee(candidate.to_stash(), None).await?;
        jury.judge_reward_destination(desination);

        // Requirement: Commission.
        let prefs = self.client.validators(candidate.to_stash(), None).await?;
        jury.judge_commission(prefs.commission);

        // Requirement: Controller set.
        let controller = self.client.bonded(candidate.to_stash(), None).await?;
        jury.judge_stash_controller_deviation(&controller);

        // Requirement: Bonded amount.
        if let Some(controller) = controller {
            let ledger = self.client.ledger(controller, None).await?;
            jury.judge_bonded_amount(ledger);
        } else {
            jury.judge_bonded_amount(None);
        }

        Ok(jury.generate_report())
    }
}
