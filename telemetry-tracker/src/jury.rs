use crate::Result;
use substrate_subxt::identity::{Data, Identity, IdentityOfStoreExt, Judgement, Registration};
use substrate_subxt::staking::{PayeeStoreExt, RewardDestination, Staking};
use substrate_subxt::{balances::Balances, sp_runtime::SaturatedConversion};
use substrate_subxt::{Client, ClientBuilder, Runtime};

pub struct ChainData<R: Runtime> {
    client: Client<R>,
}

impl<R: Runtime + Identity + Staking> ChainData<R> {
    pub async fn new(hostname: &str) -> Result<ChainData<R>> {
        Ok(ChainData {
            client: ClientBuilder::<R>::new().set_url(hostname).build().await?,
        })
    }
    pub async fn get_identity(
        &self,
        account: R::AccountId,
    ) -> Result<Option<Registration<R::Balance>>> {
        self.client
            .identity_of(account, None)
            .await
            .map_err(|err| err.into())
    }
    pub async fn get_payee(
        &self,
        account: R::AccountId,
    ) -> Result<RewardDestination<R::AccountId>> {
        self.client
            .payee(account, None)
            .await
            .map_err(|err| err.into())
    }
}

pub enum Field {
    JudgedByRegistrar,
    IdentityInfo,
    RewardDestination,
}

pub enum Compliance {
    Ok(Field),
    Err(Field),
}

use std::marker::PhantomData;

pub struct RequirementsJudgement<T> {
    compliances: Vec<Compliance>,
    _p: PhantomData<T>,
}

impl<T: Runtime + Balances> RequirementsJudgement<T> {
    fn new() -> Self {
        RequirementsJudgement {
            compliances: vec![],
            _p: PhantomData,
        }
    }
    fn judge_identity(&mut self, identity: Registration<T::Balance>) {
        // Check whether the identity has been judged by a registrar.
        let mut is_judged = false;
        for (_, judgement) in identity.judgements {
            if judgement == Judgement::Reasonable || judgement == Judgement::Reasonable {
                is_judged = true;
                break;
            }
        }

        match is_judged {
            true => self
                .compliances
                .push(Compliance::Ok(Field::JudgedByRegistrar)),
            false => self
                .compliances
                .push(Compliance::Err(Field::JudgedByRegistrar)),
        }

        // Check whether the identity has the display name and email field set.
        let info = identity.info;
        if info.display != Data::None && info.email != Data::None {
            self.compliances.push(Compliance::Ok(Field::IdentityInfo));
        } else {
            self.compliances.push(Compliance::Err(Field::IdentityInfo));
        }
    }
    fn judge_reward_destination(&mut self, reward_destination: RewardDestination<T::AccountId>) {
        if reward_destination == RewardDestination::Staked {
            self.compliances
                .push(Compliance::Ok(Field::RewardDestination));
        } else {
            self.compliances
                .push(Compliance::Ok(Field::RewardDestination));
        }
    }
}

pub struct RankingJudgement {}

impl RankingJudgement {
    fn new() -> Self {
        RankingJudgement {}
    }
}
