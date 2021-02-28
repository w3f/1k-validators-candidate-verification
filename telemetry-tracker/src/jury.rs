use crate::Result;
use substrate_subxt::identity::{Data, Identity, IdentityOfStoreExt, Judgement, Registration};
use substrate_subxt::{Client, ClientBuilder, Runtime};

pub struct ChainData<R: Runtime> {
    client: Client<R>,
}

impl<R: Runtime + Identity> ChainData<R> {
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
}

pub enum Field {
    JudgedByRegistrar,
    IdentityInfo,
}

pub enum Compliance {
    Ok(Field),
    Err(Field),
}

pub struct RequirementsJudgement {
    compliances: Vec<Compliance>,
}

impl RequirementsJudgement {
    fn new() -> Self {
        RequirementsJudgement {
            compliances: vec![],
        }
    }
    fn judge_identity(&mut self, identity: Registration<u128>) {
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
}

pub struct RankingJudgement {}

impl RankingJudgement {
    fn new() -> Self {
        RankingJudgement {}
    }
}
