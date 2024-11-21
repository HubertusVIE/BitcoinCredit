use crate::service::ServiceContext;
use log::info;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::fs::FileServer;
use rocket::http::Header;
use rocket::{catch, catchers, routes, Build, Config, Request, Response, Rocket};

pub mod data;
mod handlers;

use crate::constants::MAX_FILE_SIZE_BYTES;
use rocket::data::ByteUnit;
use rocket::figment::Figment;

pub fn rocket_main(context: ServiceContext) -> Rocket<Build> {
    let conf = context.config.clone();
    let config = Figment::from(Config::default())
        .merge(("limits.forms", ByteUnit::Byte(MAX_FILE_SIZE_BYTES as u64)))
        .merge(("limits.file", ByteUnit::Byte(MAX_FILE_SIZE_BYTES as u64)))
        .merge((
            "limits.data-form",
            ByteUnit::Byte(MAX_FILE_SIZE_BYTES as u64),
        ))
        .merge(("port", conf.http_port))
        .merge(("address", conf.http_address.to_owned()));

    let rocket = rocket::custom(config)
        .register("/", catchers![not_found])
        .manage(context)
        .mount("/exit", routes![handlers::exit])
        .mount("/opcodes", routes![handlers::return_operation_codes])
        .mount(
            "/identity",
            routes![
                handlers::identity::create_identity,
                handlers::identity::change_identity,
                handlers::identity::return_identity,
                handlers::identity::return_peer_id
            ],
        )
        .mount("/bitcredit", FileServer::from("frontend_build"))
        .mount(
            "/contacts",
            routes![
                handlers::contacts::new_contact,
                handlers::contacts::edit_contact,
                handlers::contacts::remove_contact,
                handlers::contacts::return_contacts
            ],
        )
        .mount(
            "/company",
            routes![
                handlers::company::list,
                handlers::company::detail,
                handlers::company::upload_file,
            ],
        )
        .mount(
            "/bill",
            routes![
                handlers::bill::holder,
                handlers::bill::attachment,
                handlers::bill::issue_bill,
                handlers::bill::upload_files,
                handlers::bill::endorse_bill,
                handlers::bill::search_bill,
                handlers::bill::request_to_accept_bill,
                handlers::bill::accept_bill,
                handlers::bill::request_to_pay_bill,
                handlers::bill::return_bill,
                handlers::bill::return_chain_of_blocks,
                handlers::bill::return_basic_bill,
                handlers::bill::sell_bill,
                handlers::bill::mint_bill,
                handlers::bill::accept_mint_bill,
                handlers::bill::find_bill_in_dht,
                handlers::bill::request_to_mint_bill,
            ],
        )
        .mount("/bills", routes![handlers::bill::return_bills_list,])
        .mount(
            "/quote",
            routes![
                handlers::quotes::return_quote,
                handlers::quotes::accept_quote
            ],
        )
        .attach(Cors);

    info!("HTTP Server Listening on {}", conf.http_listen_url());

    match open::that(format!("{}/bitcredit/", conf.http_listen_url()).as_str()) {
        Ok(_) => {}
        Err(_) => {
            info!("Can't open browser.")
        }
    }

    rocket
}

struct Cors;

#[rocket::async_trait]
impl Fairing for Cors {
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers to responses",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, _request: &'r Request<'_>, response: &mut Response<'r>) {
        response.set_header(Header::new("Access-Control-Allow-Origin", "*"));
        response.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "POST, GET, PATCH, OPTIONS, PUT, DELETE",
        ));
        response.set_header(Header::new("Access-Control-Allow-Headers", "*"));
        response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
    }
}

#[catch(404)]
pub fn not_found(req: &Request) -> String {
    format!("We couldn't find the requested path '{}'", req.uri())
}
