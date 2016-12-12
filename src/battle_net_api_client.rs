//! A Module for accessing Blizzard's WoW API.
//! The exposed functionality of this module has been structured
//! around the particular needs of blood-money: Not all fields are
//! represented and it's probably not generally useful.
//! TODO: Add support for locales.
extern crate hyper;
extern crate serde_json;

use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::time::Duration;

use hyper::client::{Client, Response};
use serde::de::Deserialize;
use thread_throttler::ThreadThrottler;

/// The content we care about in the realm status response.
#[derive(Debug, Serialize, Deserialize)]
pub struct RealmInfo {
    pub name: String,
    pub slug: String,
    pub connected_realms: Vec<String>,
}

/// Content we care about in an item info response.
#[derive(Debug, Deserialize)]
pub struct ItemInfo {
    pub id: u64,
    pub name: String,
    pub icon: String,
}

pub struct BattleNetApiClient {
    pub token: String,
    client: Client,
    tt: ThreadThrottler,
}

impl BattleNetApiClient {
    pub fn new(token: &str) -> BattleNetApiClient {
        BattleNetApiClient {
            token: token.to_owned(),
            client: Client::new(),
            tt: ThreadThrottler::new(100, Duration::new(1, 0)),
        }
    }

    /// Try to retrieve something from the Blizzard API. Will retry indefinitely.
    /// Returns the body as a String.
    /// `task` will be used for error messages.
    /// TODO: Really this should try to decode the json as well and be type
    /// inferred from context.
    fn make_blizzard_api_call<T: Deserialize>(&self, url: &str, task: &str) -> T {
        let mut s = String::new();
        let mut retries = 0;

        loop {
            let mut res: Response;
            retries += 1;

            self.tt.pass_through_or_block();
            match self.client.get(url).send() {
                Ok(r) => res = r,
                Err(e) => {
                    println!("Error downloading {}: {}. Retry {}.", task, e, retries);
                    continue;
                },
            }
            if res.status != hyper::Ok {
                println!("Error downloading {}: {}. Retry {}.", task, res.status, retries);
                continue;
            }
            match res.read_to_string(&mut s) {
                Ok(_) => (),
                Err(e) => {
                    println!("Failed to process {}: {}. Retry {}.", task, e, retries);
                    continue;
                },
            }
            match serde_json::from_str(&s) {
                Ok(obj) => return obj,
                Err(e) => {
                    println!("Failed to decode json for {}: {}. Retry {}.", task, e, retries);
                },
            }
        }
    }

    /// Downloads a list of realms from the Blizzard API.

    pub fn get_realms(&self) -> Vec<RealmInfo> {
        let mut realm_data: BTreeMap<String, Vec<RealmInfo>> =
            self.make_blizzard_api_call(&format!("https://us.api.battle.net/wow/realm/status?locale=en_US&apikey={}", self.token), "realm status");
        realm_data.remove("realms").expect("Malformed realm response.")
    }

    /// Helpler function to process a vec of RealmInfo's into sets of connected realms.
    /// Connected realms share an auction house.
    pub fn process_realm_sets(realm_infos: &Vec<RealmInfo>) -> Vec<Vec<String>> {
        let mut realm_sets: Vec<Vec<String>> = realm_infos.into_iter().map(|r|
            r.connected_realms.clone()
        ).collect();

        // This dedup logic relies on the ordering within a connected realms list being the same
        // for all realms in the list.
        realm_sets.sort_by(|a, b| a.iter().next().unwrap().cmp(b.iter().next().unwrap()));
        realm_sets.dedup();
        return realm_sets;
    }

    pub fn get_item_info(&self, id: u64) -> ItemInfo {
        self.make_blizzard_api_call::<ItemInfo>(&format!("https://us.api.battle.net/wow/item/{}?locale=en_US&apikey={}", id, self.token), "item info")
    }
}
