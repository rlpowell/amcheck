#![warn(clippy::all, clippy::pedantic)]
use regex::Regex;
use secrecy::ExposeSecret;
use std::env;

use amcheck::configuration::{get_configuration, get_environment, Matcher, MatcherPart};

use tracing::dispatcher::{self, Dispatch};
use tracing::{debug, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

#[tracing::instrument]
fn main() {
    let subscriber = Registry::default()
        .with(
            tracing_logfmt::builder()
                .with_span_path(true)
                .with_level(true)
                .with_target(true)
                .with_span_name(true)
                .layer(),
        )
        .with(EnvFilter::from_default_env());

    dispatcher::set_global_default(Dispatch::new(subscriber))
        .expect("Global logger has already been set!");

    let settings = get_configuration().expect("Failed to read configuration.");

    debug!("Settings: {:?}", settings);

    let args: Vec<String> = env::args().collect();

    let mut client = imap::ClientBuilder::new(settings.basics.imapserver, 993);
    if get_environment() == amcheck::configuration::Environment::Test {
        // DANGER: do not use in prod!
        client = client.danger_skip_tls_verify(true);
    }
    let client = client.connect().unwrap();

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = client
        .login(
            settings.basics.login,
            settings.basics.password.expose_secret(),
        )
        .expect("Can't authenticate.");
    // .map_err(|e| e.0)?;

    match args[1].as_str() {
        "move" => {
            move_to_storage(&mut imap_session, settings.basics.matcher_sets, false);
        }
        "move_noop" => {
            move_to_storage(&mut imap_session, settings.basics.matcher_sets, true);
        }
        "check" => {
            check_storage(&imap_session, settings.basics.matcher_sets);
        }
        _ => panic!("Sole argument must be either 'move' or 'check'."),
    }

    // be nice to the server and log out
    imap_session.logout().unwrap();
}

#[tracing::instrument]
fn addresses_to_string(addresses: &Vec<imap_proto::Address>) -> String {
    addresses
        .iter()
        .map(address_to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

#[tracing::instrument]
fn address_to_string(address: &imap_proto::Address) -> String {
    let name = match address.name.as_ref() {
        Some(x) => format!(
            "\"{}\" ",
            std::str::from_utf8(x).expect("Couldn't convert a name in an Address to UTF-8")
        ),
        None => String::new(),
    };
    let mailbox = std::str::from_utf8(
        address
            .mailbox
            .as_ref()
            .expect("Couldn't get a mailbox out of an Address"),
    )
    .expect("Couldn't convert a mailbox in an Address to UTF-8")
    .to_string();
    let host = std::str::from_utf8(
        address
            .host
            .as_ref()
            .expect("Couldn't get a host out of an Address"),
    )
    .expect("Couldn't convert a host in an Address to UTF-8")
    .to_string();
    format!("{}<{}@{}>", name, mailbox, host)
}

fn check_matcher_part(mp: MatcherPart, from_addr: &String, subject: &String) -> bool {
    match mp {
        MatcherPart::From(regex) => {
            if regex.is_match(from_addr.as_str()) {
                return true;
            }
        }
        MatcherPart::Subject(regex) => {
            if regex.is_match(subject.as_str()) {
                return true;
            }
        }
    }

    false
}

#[tracing::instrument]
fn match_mail(matcher_set: &Vec<Matcher>, from_addr: &String, subject: &String) -> bool {
    for matcher in matcher_set.iter() {
        match matcher {
            Matcher::Match(match_part) => {
                if !check_matcher_part(match_part.clone(), from_addr, subject) {
                    return false;
                }
            }
            Matcher::UnMatch(match_part) => {
                if check_matcher_part(match_part.clone(), from_addr, subject) {
                    return false;
                }
            }
        }
    }

    println!(
        "Mail with from addr \n\t{}\n and subject \n\t{}\n was matched by \n\t{:?}\n",
        from_addr, subject, matcher_set
    );
    return true;
}

#[tracing::instrument]
fn check_storage(
    imap_session: &imap::Session<Box<dyn imap::ImapConnection>>,
    matcher_sets: Vec<Vec<Matcher>>,
) {
    todo!("Check not implemented.")
}

#[tracing::instrument(skip(matcher_sets,imap_session), fields(matcher_sets_count = matcher_sets.len()))]
fn move_to_storage(
    imap_session: &mut imap::Session<Box<dyn imap::ImapConnection>>,
    matcher_sets: Vec<Vec<Matcher>>,
    noop: bool,
) {
    imap_session.select("INBOX").unwrap();

    #[allow(clippy::needless_late_init)]
    let odt_formatted: String;

    if get_environment() == amcheck::configuration::Environment::Test {
        // For testing, just statically go back far enough to cover everything that's in there
        odt_formatted = "SINCE 01-Jan-2000".to_string();
    } else {
        // Generate a date like "SINCE 02-Sep-2023" that goes back 2 months-ish
        let date_format =
            time::format_description::parse("SINCE [day]-[month repr:short]-[year]").unwrap();
        let odt = time::OffsetDateTime::now_local()
            .unwrap()
            .checked_sub(time::Duration::days(60))
            .unwrap();
        odt_formatted = odt.format(&date_format).unwrap();
    }

    let seqs = imap_session
        .search(odt_formatted)
        .expect("Could not search for recent messages!");

    let seqs_list = seqs
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let messages = imap_session
        .fetch(seqs_list, "ENVELOPE")
        .expect("Couldn't fetch messages!");

    let mut storables = Vec::new();

    for message in messages.iter() {
        let envelope = message
            .envelope()
            .expect("message did not have an envelope!");
        let from_addr = addresses_to_string(
            envelope
                .from
                .as_ref()
                .expect("message envelope did not have a from address!"),
        );
        // println!("from: {}", from_addr);
        let subject = match envelope.subject {
            Some(ref x) => std::str::from_utf8(x)
                .expect("Message Subject was not valid utf-8")
                .to_string(),
            None => "".to_string(),
        };
        // let subject = std::str::from_utf8(envelope.subject.as_ref().expect("Envelope did not have a subject!"))
        //     .expect("Message Subject was not valid utf-8").to_string();
        // println!("subject: {}", subject);

        for matcher_set in &matcher_sets {
            if match_mail(matcher_set, &from_addr, &subject) {
                storables.push(message.message.to_string());
                break;
            }
        }
    }

    if storables.is_empty() {
        info!("No mails to move.");
    } else if noop {
        info!(
            "In no-op mode, doing nothing with {} mails.",
            storables.len()
        );
    } else {
        // Check if the amcheck_storage folder exists
        let names = imap_session
            .list(Some(""), Some("amcheck_storage"))
            .unwrap();

        // for name in names.iter() {
        //     println!("name: {:?}", name);
        // }

        if names.is_empty() {
            info!("amcheck_storage doesn't exist, creating");
            imap_session.create("amcheck_storage").unwrap();
        }

        info!("Moving {} mails to storage.", storables.len());
        // FIXME: .expect loses the actual output
        let sessout = imap_session
            .mv(storables.join(","), "amcheck_storage")
            .expect("Move to storage failed!");
        info!("Moved to storage: {:?}", sessout);
    }
}
