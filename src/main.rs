#![warn(clippy::all, clippy::pedantic)]
use core::panic;
use secrecy::ExposeSecret;
use std::cmp::Ordering;
use std::env;

use amcheck::configuration::{
    get_configuration, get_environment, Action, CheckerTree, Matcher, MatcherPart, MatcherSet,
};

use amcheck::my_imap_wrapper::{my_uid_search, Uid};

use error_stack::{Result, ResultExt};
use thiserror::Error;

use tracing::{debug, enabled, error, info, warn, Level};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Error)]
enum MyError {
    #[error("IMAP error")]
    Imap,
    #[error("Mail format error: {0}")]
    MailFormat(&'static str),
    #[error("Date formatting error")]
    DateFormatting,
    #[error("Could not subtract {0} days from now.")]
    DateSubtraction(i64),
}

#[tracing::instrument]
fn main() -> Result<(), MyError> {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_target(true)
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .init();

    let settings = get_configuration().expect("Failed to read configuration.");

    debug!("Settings: {:?}", settings);

    let args: Vec<String> = env::args().collect();

    let mut client = imap::ClientBuilder::new(settings.imapserver, 993);
    if get_environment() == amcheck::configuration::Environment::Test {
        // DANGER: do not use in prod!
        client = client.danger_skip_tls_verify(true);
    }
    let client = client.connect().change_context(MyError::Imap)?;

    // The client we have here is unauthenticated;
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = client
        .login(settings.login, settings.password.expose_secret())
        .expect("Can't authenticate.");

    if enabled!(Level::TRACE) {
        // At trace level, show all communications
        imap_session.debug = true;
    }

    match args[1].as_str() {
        "move" => {
            move_to_storage(&mut imap_session, settings.matcher_sets, false)?;
        }
        "move_noop" => {
            move_to_storage(&mut imap_session, settings.matcher_sets, true)?;
        }
        "check" => {
            check_storage(&mut imap_session, settings.matcher_sets, false)?;
        }
        "check_noop" => {
            check_storage(&mut imap_session, settings.matcher_sets, true)?;
        }
        _ => panic!("Sole argument must be either 'move' or 'check'."),
    }

    // Be nice to the server and log out
    imap_session.logout().change_context(MyError::Imap)?;

    Ok(())
}

#[tracing::instrument]
fn addresses_to_string(addresses: Option<&Vec<imap_proto::Address>>) -> Result<String, MyError> {
    match addresses {
        Some(x) => Ok(x
            .iter()
            .map(address_to_string)
            .collect::<Result<Vec<_>, _>>()?
            .join(", ")),
        None => Err(MyError::MailFormat("No addresses found").into()),
    }
}

#[tracing::instrument]
fn address_to_string(address: &imap_proto::Address) -> Result<String, MyError> {
    let name = match address.name.as_ref() {
        Some(x) => std::str::from_utf8(x).change_context(MyError::MailFormat(
            "Couldn't convert a name in an Address to UTF-8",
        ))?,
        None => "",
    };

    let address_lp = match address.mailbox.as_ref() {
        Some(x) => x,
        // Couldn't get this to trigger in testing
        None => Err(MyError::MailFormat("Address Local Part Missing"))?,
    };

    let localpart = std::str::from_utf8(address_lp)
        .change_context(MyError::MailFormat(
            "Couldn't convert a local part ('mailbox') in an Address to UTF-8",
        ))?
        .to_string();

    let address_host = match address.host.as_ref() {
        Some(x) => x,
        // Couldn't get this to trigger in testing
        None => Err(MyError::MailFormat("Address Host Missing"))?,
    };

    let host = std::str::from_utf8(address_host)
        .change_context(MyError::MailFormat(
            "Couldn't convert a host in an Address to UTF-8",
        ))?
        .to_string();

    Ok(format!("\"{name}\" <{localpart}@{host}>"))
}

fn check_matcher_part(
    mp: MatcherPart,
    imap_session: &mut imap::Session<Box<dyn imap::ImapConnection>>,
    uid: Uid,
    from_addr: &str,
    subject: &str,
) -> bool {
    match mp {
        MatcherPart::From(regex) => {
            if regex.is_match(from_addr) {
                return true;
            }
        }
        MatcherPart::Subject(regex) => {
            if regex.is_match(subject) {
                return true;
            }
        }
        MatcherPart::Body(regex) => {
            let mails = imap_session
                .uid_fetch(uid.to_string(), "BODY.PEEK[TEXT]")
                .expect("Couldn't fetch mail for body check!");

            if mails.len() != 1 {
                error!("Couldn't retrieve mail by uid {uid}: from_addr: {from_addr}, subject: {subject}.");
                for mail in mails.iter() {
                    error!("Mail: {mail:#?}");
                }
                panic!();
            }

            let body = match mails.get(0).unwrap().text() {
                Some(x) => std::str::from_utf8(x)
                    .unwrap_or_else(|err| {
                        panic!("Mail body was not valid utf-8\n\nerror is: {err}\n\nfrom_addr: {from_addr}, subject: {subject}")
                    })
                    .to_string(),
                // Was not able to trigger this in testing
                None => panic!("mail has no body\n\nfrom_addr: {from_addr}, subject: {subject}")
            };

            if regex.is_match(&body) {
                return true;
            }

            if enabled!(Level::DEBUG) {
                println!("Non-matching mail body: {body}");
            }
        }
    }

    false
}

#[tracing::instrument]
fn match_mail(
    name: &str,
    matchers: &Vec<Matcher>,
    imap_session: &mut imap::Session<Box<dyn imap::ImapConnection>>,
    uid: Uid,
    from_addr: &String,
    subject: &String,
) -> bool {
    for matcher in matchers {
        match matcher {
            Matcher::Match(match_part) => {
                if !check_matcher_part(match_part.clone(), imap_session, uid, from_addr, subject) {
                    return false;
                }
            }
            Matcher::UnMatch(match_part) => {
                if check_matcher_part(match_part.clone(), imap_session, uid, from_addr, subject) {
                    return false;
                }
            }
        }
    }

    if enabled!(Level::DEBUG) {
        println!(
            "Mail with from addr \n\t{from_addr}\n and subject \n\t{subject}\n was matched by {name}\n"
        );
    }
    return true;
}

#[tracing::instrument(skip(mail))]
fn get_match_data<'a>(
    mail: &'a imap::types::Fetch,
) -> Option<(String, String, time::OffsetDateTime)> {
    let envelope = mail.envelope().expect("Mail did not have an envelope!");

    //let mail_id = match envelope.mail_id {
    //    Some(ref x) => std::str::from_utf8(x)
    //        .expect("Mail ID was not valid utf-8")
    //        .to_string(),
    //    None => format!("NO MAIL ID FOUND for uid {}", mail.uid).to_owned(),
    //};

    let from_addr_temp = addresses_to_string(envelope.from.as_ref());

    let subject_temp = match envelope.subject {
        Some(ref x) => std::str::from_utf8(x)
            .change_context(MyError::MailFormat("Mail Subject was not valid utf-8")),
        None => Err(MyError::MailFormat("No Subject found").into()),
    };

    let raw_date = match envelope.date {
        Some(ref x) => std::str::from_utf8(x)
            .change_context(MyError::MailFormat("Mail Date was not valid utf-8")),
        None => Err(MyError::MailFormat("No Date found").into()),
    };
    let date_temp = match raw_date {
        Ok(x) => time::OffsetDateTime::parse(x, &time::format_description::well_known::Rfc2822)
            .change_context(MyError::DateFormatting),
        Err(x) => Err(x),
    };

    if let (Ok(from_addr), Ok(subject), Ok(date)) = (&from_addr_temp, &subject_temp, &date_temp) {
        Some((from_addr.clone(), (*subject).to_string(), *date))
    } else {
        let from_addr_str = match from_addr_temp {
            Err(x) => x.to_string(),
            Ok(x) => x,
        };
        let subject_str = match subject_temp {
            Err(x) => x.to_string(),
            Ok(x) => x.to_string(),
        };
        let date_str = match date_temp {
            Err(x) => x.to_string(),
            Ok(x) => x.to_string(),
        };
        warn!("Bad email, skipping: from_addr: '{from_addr_str}', subject: {subject_str}, date: {date_str}");
        None
    }
}

#[tracing::instrument(skip(matcher_sets,imap_session), fields(matcher_sets_count = matcher_sets.len()))]
fn move_to_storage(
    imap_session: &mut imap::Session<Box<dyn imap::ImapConnection>>,
    matcher_sets: Vec<MatcherSet>,
    noop: bool,
) -> Result<(), MyError> {
    imap_session.select("INBOX").change_context(MyError::Imap)?;

    #[allow(clippy::needless_late_init)]
    let odt_formatted: String;

    if get_environment() == amcheck::configuration::Environment::Test {
        // For testing, just statically go back far enough to cover everything that's in there
        odt_formatted = "SINCE 01-Jan-2000".to_string();
    } else {
        // Generate a date like "SINCE 02-Sep-2023" that goes back 2 months-ish
        let date_format = time::format_description::parse("SINCE [day]-[month repr:short]-[year]")
            .change_context(MyError::DateFormatting)?;
        let days_back: i64 = 60;
        let odt = time::OffsetDateTime::now_local()
            .change_context(MyError::DateFormatting)?
            .checked_sub(time::Duration::days(days_back))
            .ok_or(MyError::DateSubtraction(days_back))?;
        odt_formatted = odt
            .format(&date_format)
            .change_context(MyError::DateFormatting)?;
    }

    debug!("IMAP search string: {odt_formatted}");

    let uids =
        my_uid_search(imap_session, odt_formatted).expect("Could not search for recent mails!");

    let uids_list = uids
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");

    debug!("IMAP search results: {uids_list}");

    let mails = imap_session
        .uid_fetch(uids_list, "ENVELOPE")
        .expect("Couldn't fetch mails!");

    let mut storables = Vec::new();

    for mail in mails.iter() {
        if let Some((from_addr, subject, _)) = get_match_data(mail) {
            let uid = mail.uid.expect("Mail has no UID").into();
            debug!("Mail uid {uid}: from {from_addr}, subj {subject}");

            for matcher_set in &matcher_sets {
                if match_mail(
                    &matcher_set.name,
                    &matcher_set.matchers,
                    imap_session,
                    uid,
                    &from_addr,
                    &subject,
                ) {
                    info!("Marking mail to move to storage from set {}: From {from_addr}, subj {subject}", matcher_set.name);
                    storables.push(mail.uid.expect("Mail has no UID").to_string());
                    break;
                }
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
            .change_context(MyError::Imap)?;

        if names.is_empty() {
            info!("amcheck_storage doesn't exist, creating");
            imap_session
                .create("amcheck_storage")
                .change_context(MyError::Imap)?;
        }

        info!("Moving {} mails to storage.", storables.len());
        // imap.mv returns no information at all except failure
        imap_session
            .uid_mv(storables.join(","), "amcheck_storage")
            .change_context(MyError::Imap)?;
    }

    Ok(())
}

#[tracing::instrument(skip(matcher_sets, imap_session))]
fn check_storage(
    imap_session: &mut imap::Session<Box<dyn imap::ImapConnection>>,
    matcher_sets: Vec<MatcherSet>,
    noop: bool,
) -> Result<(), MyError> {
    imap_session
        .select("amcheck_storage")
        .change_context(MyError::Imap)?;

    let uids = my_uid_search(imap_session, "ALL").expect("Could not search for recent mails!");

    let mut uids_vec_temp = uids.iter().collect::<Vec<&Uid>>();

    uids_vec_temp.sort_by(|a, b| b.cmp(a));

    let uids_vec = uids_vec_temp
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<String>>();

    let uids_list = if uids_vec.len() > 100 {
        debug!(
            "IMAP search results too long at {}, trimming to most recent 2000",
            uids_vec.len()
        );
        uids_vec[..2000].join(",")
    } else {
        uids_vec.join(",")
    };

    debug!("IMAP search results: {uids_list}");

    let mails = imap_session
        .uid_fetch(uids_list, "ENVELOPE")
        .expect("Couldn't fetch mails!");

    // Walk through the list of checks
    for matcher_set in &matcher_sets {
        let mut checkables = Vec::new();

        debug!("Running matches for {}", matcher_set.name);

        // Check mails for basic matching; are these the ones we care about for this check?
        //
        // This means we run over every mail once for each check, but that can't really be helped.
        for mail in mails.iter() {
            if let Some((from_addr, subject, _)) = get_match_data(mail) {
                let uid = mail.uid.expect("Mail has no UID").into();
                debug!("Mail uid {uid}: from {from_addr}, subj {subject}");

                if match_mail(
                    &matcher_set.name,
                    &matcher_set.matchers,
                    imap_session,
                    uid,
                    &from_addr,
                    &subject,
                ) {
                    checkables.push(mail);
                }
            }
        }

        // Now that we have the mails that are relevant to this check, actually run the check(s)

        if enabled!(Level::DEBUG) {
            println!("\n\n-------------------------------------\n\n");
        }

        run_check_tree(
            noop,
            &matcher_set.name,
            &matcher_set.checker_tree,
            imap_session,
            &checkables,
        )?;
    }

    Ok(())
}

fn print_head_of(tree: &CheckerTree) -> String {
    match tree {
        CheckerTree::Empty => "Empty".to_string(),
        CheckerTree::Action(x) => format!("Action {x:?}"),
        CheckerTree::MatchCheck(x) => format!("MatchCheck {:?}", x.matchers),
        CheckerTree::DateCheck(x) => format!("DateCheck {}", x.days),
        CheckerTree::CountCheck(x) => format!("CountCheck {}", x.count),
    }
}

#[tracing::instrument(skip(checker_tree, mails, imap_session), fields(tree_head_type = print_head_of(checker_tree)))]
fn run_check_tree(
    noop: bool,
    name: &str,
    checker_tree: &CheckerTree,
    imap_session: &mut imap::Session<Box<dyn imap::ImapConnection>>,
    mails: &Vec<&imap::types::Fetch>,
) -> Result<(), MyError> {
    match checker_tree {
        CheckerTree::Empty => {}
        CheckerTree::Action(action) => match action {
            Action::Alert => {
                if noop {
                    println!(
                        "In noop mode, not alerting for check '{name}' for {} mails",
                        mails.len()
                    );
                } else if !mails.is_empty() {
                    warn!("CHECK FAILED for check '{name}' for {} mails", mails.len());
                    for mail in mails {
                        if let Some((from_addr, subject, date)) = get_match_data(mail) {
                            warn!("CHECK FAILED DETAILS for check '{name}': mail from '{from_addr}' with subject '{subject}' and date '{date}'!");
                        }
                    }
                }
            }
            Action::Nothing => {}
            Action::Log => {
                info!("Check '{name}' passed with {} mails", mails.len());
            }
            Action::Delete => {
                info!("Deleting {} mails.", mails.len());
                let uids_list = mails
                    .iter()
                    .map(|x| x.uid.expect("Mail has no UID").to_string())
                    .collect::<Vec<_>>()
                    .join(",");

                if noop {
                    println!("In noop mode, not deleting uids {uids_list:#?}");
                } else {
                    imap_session
                        .uid_store(uids_list, "+FLAGS (\\Deleted)")
                        .change_context(MyError::Imap)?;
                    imap_session.expunge().change_context(MyError::Imap)?;
                }
            }
        },
        CheckerTree::MatchCheck(check) => {
            // Given the matchers, build a list of mails that do and do not match
            let mut matched = Vec::new();
            let mut not_matched = Vec::new();

            for mail in mails {
                if let Some((from_addr, subject, _)) = get_match_data(mail) {
                    let uid = mail.uid.expect("Mail has no UID").into();

                    if match_mail(
                        name,
                        &check.matchers,
                        imap_session,
                        uid,
                        &from_addr,
                        &subject,
                    ) {
                        matched.push(*mail);
                    } else {
                        not_matched.push(*mail);
                    }
                }
            }

            // Dispatch the two lists down the tree
            run_check_tree(noop, name, &check.matched, imap_session, &matched)?;

            run_check_tree(noop, name, &check.not_matched, imap_session, &not_matched)?;
        }
        CheckerTree::DateCheck(check) => {
            // Build a list of mails with dates before and after the given number of days ago
            let mut older = Vec::new();
            let mut younger = Vec::new();

            for mail in mails {
                if let Some((from_addr, subject, mail_date)) = get_match_data(mail) {
                    let uid: Uid = mail.uid.expect("Mail has no UID").into();

                    let odt = time::OffsetDateTime::now_local()
                        .change_context(MyError::DateFormatting)?
                        .checked_sub(time::Duration::days(i64::from(check.days)))
                        .ok_or(MyError::DateSubtraction(i64::from(check.days)))?;

                    debug!("Checking if mail date {mail_date:?} for uid {uid}: from_addr: {from_addr}, subject: {subject} is older/younger than target date {odt:?}");

                    // An older mail has a smaller date, so smaller mail_date is true
                    if mail_date < odt {
                        older.push(*mail);
                    } else {
                        // Technically "younger or equal", but whatever
                        younger.push(*mail);
                    }
                }
            }

            // Dispatch the two lists down the tree
            run_check_tree(noop, name, &check.older_than, imap_session, &older)?;

            run_check_tree(noop, name, &check.younger_than, imap_session, &younger)?;
        }
        CheckerTree::CountCheck(check) => {
            match mails.len().cmp(&usize::from(check.count)) {
                Ordering::Greater => {
                    run_check_tree(noop, name, &check.greater_than, imap_session, mails)?;
                }
                Ordering::Less => {
                    run_check_tree(noop, name, &check.less_than, imap_session, mails)?;
                }
                Ordering::Equal => run_check_tree(noop, name, &check.equal, imap_session, mails)?,
            };
        }
    }

    Ok(())
}
