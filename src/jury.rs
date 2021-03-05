use crate::system::Candidate;
use crate::Result;
use crate::{
    database::{CandidateState, TelemetryEventStore},
    events::NodeId,
};
use sp_arithmetic::Perbill;

use substrate_subxt::identity::{Data, Judgement as RegistrarJudgement, Registration};
use substrate_subxt::sp_core::crypto::Ss58Codec;
use substrate_subxt::staking::{RewardDestination, StakingLedger};

use substrate_subxt::balances::Balances;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Context {
    IdentityFound,
    RegistrarJudgement,
    CorrectIdentityInfo,
    RewardDestination,
    Commission,
    ControllerFound,
    StashControllerDeviation,
    StakingLedger,
    BondedAmount,
    NodeNameFound,
    RequiredNodeUptime,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", content = "context")]
#[serde(rename_all = "snake_case")]
pub enum Judgement {
    Ok(Context),
    #[serde(rename = "nok_fault")]
    Fault(Context),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequirementsConfig<Balance> {
    pub max_commission: u32,
    pub min_bonded_amount: Balance,
    pub node_activity_timespan: u64,
    pub max_node_activity_diff: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RequirementsJudgementReport {
    judgments: Vec<Judgement>,
    faults: PrevNow<isize>,
    rank: PrevNow<isize>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PrevNow<T> {
    pre_judgement: T,
    after_judgement: T,
}

impl Default for PrevNow<isize> {
    fn default() -> Self {
        PrevNow {
            pre_judgement: 0,
            after_judgement: 0,
        }
    }
}

pub struct RequirementsJudgement<'a, T: Balances> {
    candidate: Candidate,
    judgments: Vec<Judgement>,
    config: &'a RequirementsConfig<T::Balance>,
    faults: PrevNow<isize>,
    rank: PrevNow<isize>,
    telemetry_store: TelemetryEventStore,
}

impl<'a, T: Balances> RequirementsJudgement<'a, T>
where
    T::AccountId: Ss58Codec,
{
    pub fn new(
        state: &CandidateState,
        config: &'a RequirementsConfig<T::Balance>,
        store: TelemetryEventStore,
    ) -> Result<Self> {
        let candidate = state.candidate.clone();
        let (faults, rank) = state
            .judgement_reports
            .last()
            .map(|l| (l.event.faults.clone(), l.event.rank.clone()))
            .unwrap_or((Default::default(), Default::default()));

        Ok(RequirementsJudgement {
            candidate: candidate,
            judgments: vec![],
            config: config,
            faults: faults,
            rank: rank,
            telemetry_store: store,
        })
    }
    pub fn generate_report(self) -> RequirementsJudgementReport {
        // Count faults
        let mut faults = 0;
        for comp in &self.judgments {
            match comp {
                Judgement::Fault(_) => faults += 1,
                _ => {}
            }
        }

        let mut rank = self.rank.after_judgement;

        // Faults will only influence the rank if the weren't any in the last
        // judgement process. This prevents halving ranks continuously
        // (i.e. only half ranks if in the last round the candidate had no faults).
        if self.faults.after_judgement == 0 {
            // No matter how many faults occur, just half the rank if there are faults.
            if faults > 1 {
                rank /= 2;
            }
        }

        // Increase rank if appropriate.
        if faults == 0 {
            rank += 1;
        }

        RequirementsJudgementReport {
            judgments: self.judgments,
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
            self.judgments.push(Judgement::Ok(Context::IdentityFound));
            identity
        } else {
            self.judgments
                .push(Judgement::Fault(Context::IdentityFound));
            return;
        };

        // Check whether the identity has been judged by a registrar.
        let mut is_judged = false;
        for (_, judgement) in identity.judgements {
            if judgement == RegistrarJudgement::Reasonable
                || judgement == RegistrarJudgement::KnownGood
            {
                is_judged = true;
                break;
            }
        }

        match is_judged {
            true => self
                .judgments
                .push(Judgement::Ok(Context::RegistrarJudgement)),
            false => self
                .judgments
                .push(Judgement::Fault(Context::RegistrarJudgement)),
        }

        // Check whether the identity has the display name and email field set.
        let info = identity.info;
        if info.display != Data::None && info.email != Data::None {
            self.judgments
                .push(Judgement::Ok(Context::CorrectIdentityInfo));
        } else {
            self.judgments
                .push(Judgement::Fault(Context::CorrectIdentityInfo));
        }
    }
    pub fn judge_reward_destination(
        &mut self,
        reward_destination: RewardDestination<T::AccountId>,
    ) {
        if reward_destination == RewardDestination::Staked {
            self.judgments
                .push(Judgement::Ok(Context::RewardDestination));
        } else {
            self.judgments
                .push(Judgement::Ok(Context::RewardDestination));
        }
    }
    pub fn judge_commission(&mut self, commission: Perbill) {
        if commission.deconstruct() <= (self.config.max_commission * 1_000_000) {
            self.judgments.push(Judgement::Ok(Context::Commission));
        } else {
            self.judgments.push(Judgement::Fault(Context::Commission));
        }
    }
    pub fn judge_stash_controller_deviation(
        &mut self,
        controller: &Option<T::AccountId>,
    ) -> Result<()> {
        let controller = if let Some(controller) = controller {
            self.judgments.push(Judgement::Ok(Context::ControllerFound));
            controller
        } else {
            self.judgments
                .push(Judgement::Fault(Context::ControllerFound));
            return Ok(());
        };

        if &T::AccountId::from_ss58check(self.candidate.stash_str())
            .map_err(|err| anyhow!("failed to convert candidate to T::AccountId: {:?}", err))?
            != controller
        {
            self.judgments
                .push(Judgement::Ok(Context::StashControllerDeviation));
        } else {
            self.judgments
                .push(Judgement::Fault(Context::StashControllerDeviation));
        }

        Ok(())
    }
    pub fn judge_bonded_amount(&mut self, ledger: Option<StakingLedger<T::AccountId, T::Balance>>) {
        let ledger = if let Some(ledger) = ledger {
            self.judgments.push(Judgement::Ok(Context::StakingLedger));
            ledger
        } else {
            self.judgments
                .push(Judgement::Fault(Context::StakingLedger));
            return;
        };

        if ledger.total >= self.config.min_bonded_amount {
            self.judgments.push(Judgement::Ok(Context::BondedAmount));
        } else {
            self.judgments.push(Judgement::Fault(Context::BondedAmount));
        }
    }
    pub async fn judge_node_uptime(&mut self, node_ids: &[NodeId]) -> Result<()> {
        if !node_ids.is_empty() {
            self.judgments.push(Judgement::Ok(Context::NodeNameFound));
        } else {
            self.judgments
                .push(Judgement::Fault(Context::NodeNameFound));
            return Ok(());
        }

        // Check is node Id. As soon as one of those has the required uptime,
        // the candidate is marked as compliant.
        // TODO
        let is_compliant = false;

        if is_compliant {
            self.judgments
                .push(Judgement::Ok(Context::RequiredNodeUptime));
        } else {
            self.judgments
                .push(Judgement::Fault(Context::RequiredNodeUptime));
        }

        Ok(())
    }
}
