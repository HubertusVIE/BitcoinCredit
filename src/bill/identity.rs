use borsh_derive::{BorshDeserialize, BorshSerialize};
use libp2p::identity::Keypair;
use libp2p::PeerId;
use rocket::serde::{Deserialize, Serialize};
use rocket::FromForm;
use std::fs;

use crate::constants::IDENTITY_PEER_ID_FILE_PATH;

#[derive(Clone)]
pub struct IdentityWithAll {
    pub identity: Identity,
    pub peer_id: PeerId,
    #[allow(dead_code)]
    pub key_pair: Keypair,
}

#[derive(BorshSerialize, BorshDeserialize, FromForm, Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Identity {
    pub name: String,
    pub company: String,
    pub date_of_birth: String,
    pub city_of_birth: String,
    pub country_of_birth: String,
    pub email: String,
    pub postal_address: String,
    pub public_key_pem: String,
    pub private_key_pem: String,
    pub bitcoin_public_key: String,
    pub bitcoin_private_key: String,
    pub nostr_npub: Option<String>,
}

macro_rules! update_field {
    ($self:expr, $other:expr, $field:ident) => {
        if !$other.$field.is_empty() {
            $self.$field = $other.$field.clone();
        }
    };
}

impl Identity {
    pub fn new_empty() -> Self {
        Self {
            name: "".to_string(),
            company: "".to_string(),
            date_of_birth: "".to_string(),
            city_of_birth: "".to_string(),
            bitcoin_public_key: "".to_string(),
            postal_address: "".to_string(),
            public_key_pem: "".to_string(),
            email: "".to_string(),
            country_of_birth: "".to_string(),
            private_key_pem: "".to_string(),
            bitcoin_private_key: "".to_string(),
            nostr_npub: None,
        }
    }

    fn all_changeable_fields_empty(&self) -> bool {
        self.name.is_empty()
            && self.company.is_empty()
            && self.postal_address.is_empty()
            && self.email.is_empty()
    }

    fn all_changeable_fields_equal_to(&self, other: &Self) -> bool {
        self.name == other.name
            && self.company == other.company
            && self.postal_address == other.postal_address
            && self.email == other.email
    }

    pub fn update_valid(&self, other: &Self) -> bool {
        if other.all_changeable_fields_empty() {
            return false;
        }
        if self.all_changeable_fields_equal_to(other) {
            return false;
        }
        true
    }

    pub fn update_from(&mut self, other: &Identity) {
        update_field!(self, other, name);
        update_field!(self, other, company);
        update_field!(self, other, postal_address);
        update_field!(self, other, email);
    }
}

pub fn read_peer_id_from_file() -> PeerId {
    let data: Vec<u8> =
        fs::read(IDENTITY_PEER_ID_FILE_PATH).expect("Unable to read file with peer id");

    PeerId::from_bytes(&data).expect("can deserialize peer id")
}
