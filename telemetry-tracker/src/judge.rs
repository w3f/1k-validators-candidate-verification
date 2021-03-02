use crate::database::{CandidateState, TelemetryEventStore};
use crate::system::{Candidate, Network};
use crate::Result;
use crate::{
    events::NetworkId,
    jury::{RequirementsConfig, RequirementsJudgement, RequirementsJudgementReport},
};
use std::convert::TryFrom;
use substrate_subxt::identity::{Identity, IdentityOfStoreExt};
use substrate_subxt::sp_core::crypto::{AccountId32, Ss58AddressFormat, Ss58Codec};
use substrate_subxt::staking::{
    BondedStoreExt, LedgerStoreExt, PayeeStoreExt, Staking, ValidatorsStoreExt,
};
use substrate_subxt::system::System;
use substrate_subxt::{balances::Balances, DefaultNodeRuntime, KusamaRuntime};
use substrate_subxt::{Client, ClientBuilder, Runtime};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NetworkAccount<T>(T);

impl<T: Clone> NetworkAccount<T> {
    pub fn raw(&self) -> &T {
        &self.0
    }
    pub fn to_stash(&self) -> T {
        self.0.clone()
    }
}

// TODO: This required?
pub trait ToCandidate<T> {
    fn to_candidate(self) -> Candidate;
}

impl ToCandidate<DefaultNodeRuntime> for NetworkAccount<<DefaultNodeRuntime as System>::AccountId> {
    fn to_candidate(self) -> Candidate {
        Candidate::from((
            self.raw()
                .to_ss58check_with_version(Ss58AddressFormat::PolkadotAccount),
            Network::Polkadot,
        ))
    }
}

impl ToCandidate<KusamaRuntime> for NetworkAccount<<KusamaRuntime as System>::AccountId> {
    fn to_candidate(self) -> Candidate {
        Candidate::from((
            self.raw()
                .to_ss58check_with_version(Ss58AddressFormat::KusamaAccount),
            Network::Kusama,
        ))
    }
}

impl From<AccountId32> for NetworkAccount<AccountId32> {
    fn from(val: AccountId32) -> Self {
        NetworkAccount(val)
    }
}

impl TryFrom<Candidate> for NetworkAccount<AccountId32> {
    type Error = anyhow::Error;

    fn try_from(val: Candidate) -> Result<Self> {
        Ok(NetworkAccount(
            AccountId32::from_ss58check(val.stash_str()).map_err(|err| {
                anyhow!("Failed to convert presumed SS58 string into a NetworkAccount")
            })?,
        ))
    }
}

#[allow(dead_code)]
pub struct RequirementsProceeding<T: Runtime + Balances> {
    client: Client<T>,
    requirements: RequirementsConfig<T::Balance>,
    telemetry_store: TelemetryEventStore,
}

impl<T: Runtime + Balances + Identity + Staking> RequirementsProceeding<T>
where
    T::AccountId: Ss58Codec,
{
    pub async fn new(
        rpc_hostname: &str,
        requirements: RequirementsConfig<T::Balance>,
        store: TelemetryEventStore,
    ) -> Result<Self> {
        Ok(RequirementsProceeding {
            client: ClientBuilder::<T>::new()
                .set_url(rpc_hostname)
                .skip_type_sizes_check()
                .build()
                .await?,
            requirements: requirements,
            telemetry_store: store,
        })
    }
    pub async fn proceed_requirements(
        &self,
        state: CandidateState,
    ) -> Result<RequirementsJudgementReport> {
        let mut jury = RequirementsJudgement::<T>::new(
            &state,
            &self.requirements,
            self.telemetry_store.clone(),
        )?;

        let account_id = state.candidate.to_account_id::<T::AccountId>()?;

        println!("GOT HERE");
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
        jury.judge_stash_controller_deviation(&controller);

        // Requirement: Bonded amount.
        debug!("Checking bonded amount requirement");
        if let Some(controller) = controller {
            let ledger = self.client.ledger(controller, None).await?;
            jury.judge_bonded_amount(ledger);
        } else {
            jury.judge_bonded_amount(None);
        }

        Ok(jury.generate_report())
    }
}
