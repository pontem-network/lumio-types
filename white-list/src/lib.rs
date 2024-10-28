use std::{collections::HashSet, sync::Arc, time::Duration};

use arc_swap::ArcSwap;
use log::{debug, error};
use lumio_types::h256::H256;
use reqwest::Error;
use url::Url;

type Address = H256;

pub struct WhiteList {
    accounts: ArcSwap<HashSet<Address>>,
}

impl WhiteList {
    pub fn is_account_exist<T>(&self, key: T) -> bool
    where
        T: TryInto<H256>,
    {
        let Ok(address) = key.try_into() else {
            return false;
        };
        self.accounts.load().contains(&address)
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

    pub fn new_with_list(url: Url, refresh_rate: Duration, white_list: Arc<WhiteList>) -> Self {
        Self {
            white_list,
            url,
            refresh_rate,
            http: reqwest::Client::new(),
        }
    }

    pub async fn refresh_task(self) {
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

    async fn fetch_accounts(&self) -> Result<HashSet<Address>, Error> {
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
