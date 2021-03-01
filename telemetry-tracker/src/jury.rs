use crate::judge::{NetworkAccount, ToCandidate};
use crate::system::Candidate;
use sp_arithmetic::Perbill;
use substrate_subxt::identity::{Data, Judgement, Registration};
use substrate_subxt::staking::{RewardDestination, StakingLedger};
use substrate_subxt::Runtime;
use substrate_subxt::{balances::Balances, sp_runtime::AccountId32};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    IdentityFound,
    RegistrarJudgement,
    CorrectIdentityInfo,
    RewardDestination,
    Commission,
    ControllerFound,
    StashControllerDeviation,
    StakingLedger,
    BondedAmount,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", content = "content")]
#[serde(rename_all = "snake_case")]
pub enum Compliance {
    Ok(Field),
    Err(Field),
}

pub struct RequirementsConfig<Balance> {
    pub commission: u32,
    pub bonded_amount: Balance,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RequirementsJudgementReport {
    candidate: Candidate,
    compliances: Vec<Compliance>,
}

pub struct RequirementsJudgement<'a, T: Runtime + Balances> {
    candidate: NetworkAccount<T::AccountId>,
    compliances: Vec<Compliance>,
    config: &'a RequirementsConfig<T::Balance>,
}

impl<'a, T: Runtime + Balances> RequirementsJudgement<'a, T>
where
    NetworkAccount<T::AccountId>: ToCandidate<T>,
{
    pub fn new(
        candidate: NetworkAccount<T::AccountId>,
        config: &'a RequirementsConfig<T::Balance>,
    ) -> Self {
        RequirementsJudgement {
            candidate: candidate,
            compliances: vec![],
            config: config,
        }
    }
    pub fn generate_report(self) -> RequirementsJudgementReport {
        RequirementsJudgementReport {
            candidate: ToCandidate::<T>::to_candidate(self.candidate),
            compliances: self.compliances,
        }
    }
    pub fn judge_identity(&mut self, identity: Option<Registration<T::Balance>>) {
        // Check whether the identity is available.
        let identity = if let Some(identity) = identity {
            self.compliances.push(Compliance::Ok(Field::IdentityFound));
            identity
        } else {
            self.compliances.push(Compliance::Err(Field::IdentityFound));
            return;
        };

        // Check whether the identity has been judged by a registrar.
        let mut is_judged = false;
        for (_, judgement) in identity.judgements {
            if judgement == Judgement::Reasonable || judgement == Judgement::KnownGood {
                is_judged = true;
                break;
            }
        }

        match is_judged {
            true => self
                .compliances
                .push(Compliance::Ok(Field::RegistrarJudgement)),
            false => self
                .compliances
                .push(Compliance::Err(Field::RegistrarJudgement)),
        }

        // Check whether the identity has the display name and email field set.
        let info = identity.info;
        if info.display != Data::None && info.email != Data::None {
            self.compliances
                .push(Compliance::Ok(Field::CorrectIdentityInfo));
        } else {
            self.compliances
                .push(Compliance::Err(Field::CorrectIdentityInfo));
        }
    }
    pub fn judge_reward_destination(
        &mut self,
        reward_destination: RewardDestination<T::AccountId>,
    ) {
        if reward_destination == RewardDestination::Staked {
            self.compliances
                .push(Compliance::Ok(Field::RewardDestination));
        } else {
            self.compliances
                .push(Compliance::Ok(Field::RewardDestination));
        }
    }
    pub fn judge_commission(&mut self, commission: Perbill) {
        if commission.deconstruct() <= (self.config.commission * 1_000_000) {
            self.compliances.push(Compliance::Ok(Field::Commission));
        } else {
            self.compliances.push(Compliance::Err(Field::Commission));
        }
    }
    pub fn judge_stash_controller_deviation(&mut self, controller: &Option<T::AccountId>) {
        let controller = if let Some(controller) = controller {
            self.compliances
                .push(Compliance::Ok(Field::ControllerFound));
            controller
        } else {
            self.compliances
                .push(Compliance::Err(Field::ControllerFound));
            return;
        };

        if self.candidate.raw() != controller {
            self.compliances
                .push(Compliance::Ok(Field::StashControllerDeviation));
        } else {
            self.compliances
                .push(Compliance::Err(Field::StashControllerDeviation));
        }
    }
    pub fn judge_bonded_amount(&mut self, ledger: Option<StakingLedger<T::AccountId, T::Balance>>) {
        let ledger = if let Some(ledger) = ledger {
            self.compliances.push(Compliance::Ok(Field::StakingLedger));
            ledger
        } else {
            self.compliances.push(Compliance::Err(Field::StakingLedger));
            return;
        };

        if ledger.total >= self.config.bonded_amount {
            self.compliances.push(Compliance::Ok(Field::BondedAmount));
        } else {
            self.compliances.push(Compliance::Err(Field::BondedAmount));
        }
    }
}

pub struct RankingJudgement {}

impl RankingJudgement {
    fn new() -> Self {
        RankingJudgement {}
    }
}
