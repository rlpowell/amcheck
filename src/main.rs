#![warn(clippy::all, clippy::pedantic)]
use core::panic;
use imap::types::Fetch;
use secrecy::ExposeSecret;
use std::env;

use amcheck::configuration::{
    get_configuration, get_environment, Check, CheckAction, Checker, CountLimit, DateLimit,
    Matcher, MatcherPart,
};

use error_stack::{Result, ResultExt};
use thiserror::Error;

use tracing::dispatcher::{self, Dispatch};
use tracing::{debug, enabled, info, warn, Level};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

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
            check_storage(&mut imap_session, settings.checker_sets, false)?;
        }
        "check_noop" => {
            check_storage(&mut imap_session, settings.checker_sets, true)?;
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
    seq: u32,
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
            let messages = imap_session
                .fetch(seq.to_string(), "BODY.PEEK[TEXT]")
                .expect("Couldn't fetch messages!");

            assert!(messages.len() == 1, "Couldn't retrieve message by seq {seq}: from_addr: {from_addr}, subject: {subject}.");

            let body = match messages.get(0).unwrap().text() {
                Some(x) => std::str::from_utf8(x)
                    .unwrap_or_else(|err| {
                        panic!("Message body was not valid utf-8\n\nerror is: {err}\n\nfrom_addr: {from_addr}, subject: {subject}")
                    })
                    .to_string(),
                // Was not able to trigger this in testing
                None => panic!("Message has no body\n\nfrom_addr: {from_addr}, subject: {subject}")
            };

            if regex.is_match(&body) {
                return true;
            }

            if enabled!(Level::DEBUG) {
                println!("Non-matching message body: {body}");
            }
        }
    }

    false
}

#[tracing::instrument]
fn match_mail(
    matcher_set: &Vec<Matcher>,
    imap_session: &mut imap::Session<Box<dyn imap::ImapConnection>>,
    seq: u32,
    from_addr: &String,
    subject: &String,
) -> bool {
    for matcher in matcher_set {
        match matcher {
            Matcher::Match(match_part) => {
                if !check_matcher_part(match_part.clone(), imap_session, seq, from_addr, subject) {
                    return false;
                }
            }
            Matcher::UnMatch(match_part) => {
                if check_matcher_part(match_part.clone(), imap_session, seq, from_addr, subject) {
                    return false;
                }
            }
        }
    }

    if enabled!(Level::INFO) {
        println!("Mail with from addr \n\t{from_addr}\n and subject \n\t{subject}\n was matched by \n\t{matcher_set:?}\n");
    }
    return true;
}

#[tracing::instrument(skip(message))]
fn get_match_data<'a>(
    message: &'a imap::types::Fetch,
) -> Option<(String, String, time::OffsetDateTime)> {
    let envelope = message
        .envelope()
        .expect("message did not have an envelope!");

    //let message_id = match envelope.message_id {
    //    Some(ref x) => std::str::from_utf8(x)
    //        .expect("Message ID was not valid utf-8")
    //        .to_string(),
    //    None => format!("NO MESSAGE ID FOUND for seq {}", message.message).to_owned(),
    //};

    let from_addr_temp = addresses_to_string(envelope.from.as_ref());

    let subject_temp = match envelope.subject {
        Some(ref x) => std::str::from_utf8(x)
            .change_context(MyError::MailFormat("Message Subject was not valid utf-8")),
        None => Err(MyError::MailFormat("No Subject found").into()),
    };

    let raw_date = match envelope.date {
        Some(ref x) => std::str::from_utf8(x)
            .change_context(MyError::MailFormat("Message Date was not valid utf-8")),
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
    matcher_sets: Vec<Vec<Matcher>>,
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

    let seqs = imap_session
        .search(odt_formatted)
        .expect("Could not search for recent messages!");

    let seqs_list = seqs
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");

    debug!("IMAP search results: {seqs_list}");

    let messages = imap_session
        .fetch(seqs_list, "ENVELOPE")
        .expect("Couldn't fetch messages!");

    let mut storables = Vec::new();

    for message in messages.iter() {
        if let Some((from_addr, subject, _)) = get_match_data(message) {
            let seq = message.message;
            debug!("Mail seq {seq}: from {from_addr}, subj {subject}");

            for matcher_set in &matcher_sets {
                if match_mail(matcher_set, imap_session, seq, &from_addr, &subject) {
                    storables.push(message.message.to_string());
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
            .mv(storables.join(","), "amcheck_storage")
            .change_context(MyError::Imap)?;
    }

    Ok(())
}

#[tracing::instrument(skip(imap_session, mails))]
fn perform_check_action(
    imap_session: &mut imap::Session<Box<dyn imap::ImapConnection>>,
    name: &str,
    action: &CheckAction,
    mails: &[&Fetch],
    noop: bool,
) -> Result<(), MyError> {
    match *action {
        CheckAction::Delete => {
            info!("Deleting {} mails.", mails.len());
            let seqs_list = mails
                .iter()
                .map(|x| x.message.to_string())
                .collect::<Vec<_>>()
                .join(",");

            if noop {
                println!("In noop mode, not deleting sequences {seqs_list:#?}");
            } else {
                imap_session
                    .store(seqs_list, "+FLAGS (\\Deleted)")
                    .change_context(MyError::Imap)?;
                imap_session.expunge().change_context(MyError::Imap)?;
            }
        }
        CheckAction::Alert => {
            if noop {
                println!(
                    "In noop mode, not alerting for check '{name}' for {} mails",
                    mails.len()
                );
            } else {
                warn!("CHECK FAILED for check '{name}' for {} mails", mails.len());
                for mail in mails {
                    if let Some((from_addr, subject, _)) = get_match_data(mail) {
                        warn!("CHECK FAILED DETAILS for check '{name}': mail from '{from_addr}' with subject '{subject}'!");
                    }
                }
            }
        }
    }

    Ok(())
}

fn date_matches(
    mail_date: time::OffsetDateTime,
    check_dates: &Vec<DateLimit>,
) -> Result<bool, MyError> {
    for check_date in check_dates {
        match check_date {
            DateLimit::OlderThan(days) => {
                let odt = time::OffsetDateTime::now_local()
                    .change_context(MyError::DateFormatting)?
                    .checked_sub(time::Duration::days(i64::from(*days)))
                    .ok_or(MyError::DateSubtraction(i64::from(*days)))?;

                debug!("Checking if mail date: {mail_date:?} is older than {odt:?}");

                // An older mail has a smaller date, so smaller mail_date is true
                if mail_date < odt {
                    return Ok(true);
                }
            }
            DateLimit::YoungerThan(days) => {
                let odt = time::OffsetDateTime::now_local()
                    .change_context(MyError::DateFormatting)?
                    .checked_sub(time::Duration::days(i64::from(*days)))
                    .ok_or(MyError::DateSubtraction(i64::from(*days)))?;

                debug!("Checking if mail date: {mail_date:?} is younger than {odt:?}");

                // An newer mail has a bigger date, so bigger mail_date is true
                if mail_date > odt {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

#[tracing::instrument(skip(checker_sets,imap_session), fields(checker_sets_count = checker_sets.len()))]
fn check_storage(
    imap_session: &mut imap::Session<Box<dyn imap::ImapConnection>>,
    checker_sets: Vec<Checker>,
    noop: bool,
) -> Result<(), MyError> {
    imap_session
        .select("amcheck_storage")
        .change_context(MyError::Imap)?;

    let seqs = imap_session
        .search("ALL")
        .expect("Could not search for recent messages!");

    let mut seqs_vec_temp = seqs.iter().collect::<Vec<&u32>>();

    seqs_vec_temp.sort_by(|a, b| b.cmp(a));

    let seqs_vec = seqs_vec_temp
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<String>>();

    let seqs_list = if seqs_vec.len() > 100 {
        debug!(
            "IMAP search results too long at {}, trimming to most recent 2000",
            seqs_vec.len()
        );
        seqs_vec[..2000].join(",")
    } else {
        seqs_vec.join(",")
    };

    debug!("IMAP search results: {seqs_list}");

    let messages = imap_session
        .fetch(seqs_list, "ENVELOPE")
        .expect("Couldn't fetch messages!");

    // Walk through the list of checks
    for checker_set in &checker_sets {
        let mut checkables = Vec::new();

        // Check mails for basic matching; are these the ones we care about for this check?
        //
        // This means we run over every mail once for each check, but that can't really be helped.
        for message in messages.iter() {
            if let Some((from_addr, subject, _)) = get_match_data(message) {
                let seq = message.message;
                debug!("Mail seq {seq}: from {from_addr}, subj {subject}");

                if match_mail(
                    &checker_set.matchers,
                    imap_session,
                    seq,
                    &from_addr,
                    &subject,
                ) {
                    checkables.push(message);
                }
            }
        }

        if checkables.is_empty() {
            info!("No mails to check for the '{}' check.", checker_set.name,);
        } else {
            // Now that we have the mails that are relevant to this check, actually run the check(s)

            if enabled!(Level::INFO) {
                println!("\n\n-------------------------------------\n\n");
            }

            // NOTE: the current code assumes each check is completely independent
            for check in &checker_set.checks {
                let mut matched = Vec::new();
                let mut unmatched = Vec::new();

                match check {
                    Check::CheckIndividual(check) => {
                        for message in &checkables {
                            if let Some((from_addr, subject, date)) = get_match_data(message) {
                                debug!(
                                    "Mail seq {}: from {from_addr}, subj {subject}, date {date}",
                                    message.message
                                );

                                let date_bool = date_matches(date, &checker_set.dates)?;
                                let match_bool = match_mail(
                                    &check.matchers,
                                    imap_session,
                                    message.message,
                                    &from_addr,
                                    &subject,
                                );

                                if date_bool {
                                    if match_bool {
                                        if enabled!(Level::INFO) {
                                            println!("Mail with from addr \n\t{from_addr}\n and subject \n\t{subject}\n and date \n\t{date}\n was matched by \n\t{:?} = {match_bool}\n with date check(s) {:?} = {date_bool}\n in check set {:?}\n",
                                                check.matchers, checker_set.dates, checker_set.name);
                                        }
                                        matched.push(*message);
                                    } else {
                                        if enabled!(Level::INFO) {
                                            println!("Mail with from addr \n\t{from_addr}\n and subject \n\t{subject}\n and date \n\t{date}\n was NOT matched by \n\t{:?} = {match_bool}\n with date check(s) {:?} = {date_bool}\n in check set {:?}\n",
                                                check.matchers, checker_set.dates, checker_set.name);
                                        }
                                        unmatched.push(*message);
                                    }
                                } else if enabled!(Level::INFO) {
                                    println!("Mail with from addr \n\t{from_addr}\n and subject \n\t{subject}\n and date \n\t{date}\n was NOT matched by date check(s) {:?} = {date_bool}\n in check set {:?}\n",
                                        checker_set.dates, checker_set.name);
                                }
                            }
                        }

                        if !matched.is_empty() {
                            perform_check_action(
                                imap_session,
                                &checker_set.name,
                                &check.actions.matched,
                                &matched,
                                noop,
                            )?;
                        }

                        if !unmatched.is_empty() {
                            perform_check_action(
                                imap_session,
                                &checker_set.name,
                                &check.actions.unmatched,
                                &unmatched,
                                noop,
                            )?;
                        }
                    }
                    Check::CheckCount(CountLimit::AtLeast(at_least)) => {
                        let mut count = 0;

                        for message in &checkables {
                            if let Some((from_addr, subject, date)) = get_match_data(message) {
                                if date_matches(date, &checker_set.dates)? {
                                    let seq = message.message;
                                    debug!("Mail seq {seq}: from {from_addr}, subj {subject}");

                                    count += 1;
                                }
                            }
                        }

                        if count >= *at_least {
                            info!("Check '{}' passed with {count} mails found being more than {at_least}", checker_set.name);
                        } else {
                            warn!("CHECK FAILED for check '{}' with {count} mails found being less than than {at_least}", checker_set.name);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
