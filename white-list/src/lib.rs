use std::{borrow::Borrow, collections::HashSet, sync::Arc, time::Duration};

use arc_swap::ArcSwap;
use log::{debug, error};
use reqwest::Error;
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use url::Url;

pub struct WhiteList {
    accounts: ArcSwap<HashSet<Address>>,
}

impl WhiteList {
    pub fn is_account_exist(&self, key: &Pubkey) -> bool {
        self.accounts.load().contains(key)
    }
}

impl Default for WhiteList {
    fn default() -> Self {
        Self {
            accounts: ArcSwap::new(Arc::new(HashSet::new())),
        }
    }
}

pub struct WhiteListHolder {
    white_list: Arc<WhiteList>,
    url: Url,
    http: reqwest::Client,
    refresh_rate: Duration,
}

impl WhiteListHolder {
    pub fn new(url: Url, refresh_rate: Duration) -> Self {
        Self {
            white_list: Arc::new(WhiteList::default()),
            url,
            refresh_rate,
            http: reqwest::Client::new(),
        }
    }

    pub async fn refresh_task(mut self) {
        let mut interval = tokio::time::interval(self.refresh_rate);
        loop {
            interval.tick().await;
            match self.fetch_accounts().await {
                Ok(accounts) => {
                    self.white_list.accounts.store(Arc::new(accounts));
                    debug!("Whitelist updated");
                }
                Err(err) => {
                    error!("Failed to fetch accounts: {:?}", err);
                }
            }
        }
    }

    async fn fetch_accounts(&mut self) -> Result<HashSet<Address>, Error> {
        self.http
            .get(self.url.clone())
            .send()
            .await?
            .json::<HashSet<Address>>()
            .await
    }

    pub fn get_white_list(&self) -> Arc<WhiteList> {
        Arc::clone(&self.white_list)
    }
}

#[serde_with::serde_as]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash, Deserialize)]
#[serde(transparent)]
struct Address(#[serde_as(as = "serde_with::DisplayFromStr")] Pubkey);

impl Borrow<Pubkey> for Address {
    fn borrow(&self) -> &Pubkey {
        &self.0
    }
}
