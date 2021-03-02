use crate::database::CandidateState;
use crate::judge::{NetworkAccount, ToCandidate};
use crate::system::Candidate;
use crate::Result;
use sp_arithmetic::Perbill;
use std::convert::TryFrom;
use substrate_subxt::identity::{Data, Judgement, Registration};
use substrate_subxt::sp_core::crypto::Ss58Codec;
use substrate_subxt::staking::{RewardDestination, StakingLedger};
use substrate_subxt::Runtime;
use substrate_subxt::{balances::Balances, sp_runtime::AccountId32};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
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
    Ok(Topic),
    Fault(Topic),
}

pub struct RequirementsConfig<Balance> {
    pub commission: u32,
    pub bonded_amount: Balance,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RequirementsJudgementReport {
    // TODO: Remove
    pub candidate: Candidate,
    compliances: Vec<Compliance>,
    faults: PrevNow<usize>,
    rank: PrevNow<usize>,
}

pub struct Faults(PrevNow<usize>);
pub struct Rank(PrevNow<usize>);

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PrevNow<T> {
    pre_judgement: T,
    after_judgement: T,
}

impl Default for PrevNow<usize> {
    fn default() -> Self {
        PrevNow {
            pre_judgement: 0,
            after_judgement: 0,
        }
    }
}

pub struct RequirementsJudgement<'a, T: Balances> {
    candidate: Candidate,
    compliances: Vec<Compliance>,
    config: &'a RequirementsConfig<T::Balance>,
    faults: PrevNow<usize>,
    rank: PrevNow<usize>,
}

impl<'a, T: Balances> RequirementsJudgement<'a, T>
where
    T::AccountId: Ss58Codec,
{
    pub fn new(state: CandidateState, config: &'a RequirementsConfig<T::Balance>) -> Result<Self> {
        let candidate = state.candidate;
        let (faults, rank) = state
            .requirements_report
            .last()
            .map(|l| (l.event.faults.clone(), l.event.rank.clone()))
            .unwrap_or((Default::default(), Default::default()));

        Ok(RequirementsJudgement {
            candidate: candidate,
            compliances: vec![],
            config: config,
            faults: faults,
            rank: rank,
        })
    }
    pub fn generate_report(self) -> RequirementsJudgementReport {
        // Count faults
        let mut faults = 0;
        for comp in &self.compliances {
            match comp {
                Compliance::Fault(_) => faults += 1,
                _ => {}
            }
        }

        // No matter how many faults occur.
        let mut rank = self.rank.after_judgement;
        if faults > 0 {
            rank /= 2;
        }

        RequirementsJudgementReport {
            candidate: self.candidate,
            compliances: self.compliances,
            faults: PrevNow {
                pre_judgement: self.faults.after_judgement,
                after_judgement: faults,
            },
            rank: PrevNow {
                pre_judgement: self.rank.after_judgement,
                after_judgement: rank,
            },
        }
    }
    pub fn judge_identity(&mut self, identity: Option<Registration<T::Balance>>) {
        // Check whether the identity is available.
        let identity = if let Some(identity) = identity {
            self.compliances.push(Compliance::Ok(Topic::IdentityFound));
            identity
        } else {
            self.compliances
                .push(Compliance::Fault(Topic::IdentityFound));
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
                .push(Compliance::Ok(Topic::RegistrarJudgement)),
            false => self
                .compliances
                .push(Compliance::Fault(Topic::RegistrarJudgement)),
        }

        // Check whether the identity has the display name and email field set.
        let info = identity.info;
        if info.display != Data::None && info.email != Data::None {
            self.compliances
                .push(Compliance::Ok(Topic::CorrectIdentityInfo));
        } else {
            self.compliances
                .push(Compliance::Fault(Topic::CorrectIdentityInfo));
        }
    }
    pub fn judge_reward_destination(
        &mut self,
        reward_destination: RewardDestination<T::AccountId>,
    ) {
        if reward_destination == RewardDestination::Staked {
            self.compliances
                .push(Compliance::Ok(Topic::RewardDestination));
        } else {
            self.compliances
                .push(Compliance::Ok(Topic::RewardDestination));
        }
    }
    pub fn judge_commission(&mut self, commission: Perbill) {
        if commission.deconstruct() <= (self.config.commission * 1_000_000) {
            self.compliances.push(Compliance::Ok(Topic::Commission));
        } else {
            self.compliances.push(Compliance::Fault(Topic::Commission));
        }
    }
    pub fn judge_stash_controller_deviation(
        &mut self,
        controller: &Option<T::AccountId>,
    ) -> Result<()> {
        let controller = if let Some(controller) = controller {
            self.compliances
                .push(Compliance::Ok(Topic::ControllerFound));
            controller
        } else {
            self.compliances
                .push(Compliance::Fault(Topic::ControllerFound));
            return Ok(());
        };

        //if self.candidate.try_into()? != controller {
        if &T::AccountId::from_ss58check(self.candidate.stash_str())
            .map_err(|err| anyhow!("failed to convert candidate to T::AccountId"))?
            != controller
        {
            self.compliances
                .push(Compliance::Ok(Topic::StashControllerDeviation));
        } else {
            self.compliances
                .push(Compliance::Fault(Topic::StashControllerDeviation));
        }

        Ok(())
    }
    pub fn judge_bonded_amount(&mut self, ledger: Option<StakingLedger<T::AccountId, T::Balance>>) {
        let ledger = if let Some(ledger) = ledger {
            self.compliances.push(Compliance::Ok(Topic::StakingLedger));
            ledger
        } else {
            self.compliances
                .push(Compliance::Fault(Topic::StakingLedger));
            return;
        };

        if ledger.total >= self.config.bonded_amount {
            self.compliances.push(Compliance::Ok(Topic::BondedAmount));
        } else {
            self.compliances
                .push(Compliance::Fault(Topic::BondedAmount));
        }
    }
}

pub struct RankingJudgement {}

impl RankingJudgement {
    fn new() -> Self {
        RankingJudgement {}
    }
}
