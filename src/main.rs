use imap;
use imap_proto;
use std::fs;
use std::borrow::Cow;
use std::{thread, time};
use regex::Regex;
use std::env;
use std::net::TcpStream;

#[derive(Debug)]
struct Checker {
    matches: Vec<Match>,
    unmatches: Vec<Match>,
}

#[derive(Copy,Clone,Debug)]
enum MailPart {
    Subject,
    From,
}

#[derive(Debug)]
struct Match {
    part: MailPart,
    regex: regex::Regex,
}

fn make_checker( matches: &[(MailPart, &str)], unmatches: &[(MailPart, &str)] ) -> Checker {
            Checker {
                matches: matches.iter().map( |(x,y)|
                    Match {
                        part: *x,
                        regex: Regex::new(y).unwrap(),
                    }).collect(),
                unmatches: unmatches.iter().map( |(x,y)|
                    Match {
                        part: *x,
                        regex: Regex::new(y).unwrap(),
                    }).collect()
            }

}

fn main() {
    let args: Vec<String> = env::args().collect();

    let checkers = Vec::from(
        [
            make_checker(&[(MailPart::From, " <spdrata@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <spcorpus@stodi.digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <spjbovlaste@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            // This is the hassio box; see https://bugzilla.redhat.com/show_bug.cgi?id=1845127
            make_checker(&[(MailPart::From, " <root@ibm-p8-kvm-03-guest-02.virt.pnr.lab.eng.rdu2.redhat.com>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <root@stodi.digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <puppet_cron_stodi@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <spdkhaproxy@stodi.digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <sphaproxy@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <sptwjbo@stodi.digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <spjbotcan@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <sptwdlp@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <sptwrobin@stodi.digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <sptwgaming@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <sptwrlp@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <root@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <root@[a-z]+.digitalkingdom.org>$"), (MailPart::Subject, "^Daily duplicacy backup ")], &[]),
            make_checker(&[(MailPart::From, " <root@digitalkingdom.org>$"), (MailPart::Subject, "^Daily duplicacy backup ")], &[]),
            make_checker(&[(MailPart::From, " <spjboski@lojban.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <spcamxes@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <spvlasisku@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <spmediawiki@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")], &[]),
            make_checker(&[(MailPart::From, " <rlpowell(.cron)?@digitalkingdom.org>$"), (MailPart::Subject, "^Cron ")],
                            &[(MailPart::Subject, "pic_and_video_naming")]),
            make_checker(&[(MailPart::From, " <apache@lojban.org>$"), (MailPart::Subject, "^.jvsw. (New )?(Definition|Etymology|NatLang) ")], &[]),
            make_checker(&[(MailPart::Subject, "^Report [dD]omain: digitalkingdom.org;? Submitter: ")], &[]),
            make_checker(&[(MailPart::Subject, "^Report [dD]omain: lojban.org;? Submitter: ")], &[]),
            make_checker(&[(MailPart::Subject, "^Dmarc Aggregate Report Domain: .digitalkingdom.org.")], &[]),
            make_checker(&[(MailPart::Subject, "^Dmarc Aggregate Report Domain: .lojban.org.")], &[]),
        ]
    );
    let domain = "imap.gmail.com";
    let tls = native_tls::TlsConnector::builder().build().unwrap();
    let password = fs::read_to_string("app-passwd").unwrap();
    let password = password.trim_end();

    // we pass in the domain twice to check that the server's TLS
    // certificate is valid for the domain we're connecting to.
    let client = imap::ClientBuilder::new("imap.gmail.com", 993).native_tls().unwrap();

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = client
        .login("robinleepowell@gmail.com", password)
        .expect("Can't authenticate.");
        // .map_err(|e| e.0)?;

    match args[1].as_str() {
        "move" => {
            move_to_trash(imap_session, checkers, false);
        },
        "move_noop" => {
            move_to_trash(imap_session, checkers, true);
        },
        "check" => {
            check_trash(imap_session, checkers);
        },
        _ => panic!("Sole argument must be either 'move' or 'check'."),
    }
}

fn addresses_to_string(addresses: &Vec<imap_proto::types::Address>) -> String {
    addresses.iter().map(address_to_string).collect::<Vec<_>>().join(", ")
}

fn address_to_string(address: &imap_proto::types::Address) -> String {
    let name = match address.name.as_ref() {
        Some(x) => format!("\"{}\" ", std::str::from_utf8(x).expect("Couldn't convert a name in an Address to UTF-8").to_string()),
        None => "".to_string(),
    };
    let mailbox = std::str::from_utf8(address.mailbox.as_ref().expect("Couldn't get a mailbox out of an Address"))
        .expect("Couldn't convert a mailbox in an Address to UTF-8").to_string();
    let host = std::str::from_utf8(address.host.as_ref().expect("Couldn't get a host out of an Address"))
        .expect("Couldn't convert a host in an Address to UTF-8").to_string();
    format!("{}<{}@{}>", name, mailbox, host)
}

fn match_mail(checker: &Checker, from_addr: &String, subject: &String) -> bool {
    // First do the positive matches; return false if any *don't* match
    for matchBit in &checker.matches {
        match matchBit.part {
            MailPart::From => {
                if ! matchBit.regex.is_match(from_addr.as_str()) {
                    return false;
                }
            },
            MailPart::Subject => {
                if ! matchBit.regex.is_match(subject.as_str()) {
                    return false;
                }
            },
        }
    }

    // FIXME: These two sections repeat a bunch of code

    // Then do the negative matches; return false if any *do* match
    for matchBit in &checker.unmatches {
        match matchBit.part {
            MailPart::From => {
                if matchBit.regex.is_match(from_addr.as_str()) {
                    return false;
                }
            },
            MailPart::Subject => {
                if matchBit.regex.is_match(subject.as_str()) {
                    return false;
                }
            },
        }
    }

    println!("Mail with from addr \n\t{}\n and subject \n\t{}\n was matched by \n\t{:?}\n", from_addr, subject, checker);
    return true;
}

fn check_trash(mut imap_session: imap::Session<native_tls::TlsStream<TcpStream>>, checkers: Vec<Checker>) {
    todo!("Check not implemented.")
}

fn move_to_trash(mut imap_session: imap::Session<native_tls::TlsStream<TcpStream>>, checkers: Vec<Checker>, noop: bool) {
    imap_session.select("INBOX").unwrap();

    // FIXME: Dynamic date!!
    let seqs = imap_session.search("SINCE 01-Sep-2023")
        .expect("Could not search for recent messages!");

    // let seqs_list = seqs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    let seqs_list = seqs.iter().fold("".to_string(), |acc, x| {
        if acc == "" {
            x.to_string()
        } else {
            format!("{},{}", acc, x)
        }
    });

    // let seqs_list = seqs.iter().fold("".to_string(), |acc, x| format!("{},{}", acc, x));
    let messages = imap_session.fetch(seqs_list, "ENVELOPE")
        .expect("Couldn't fetch messages!");

    let mut trashables = Vec::new();

    for message in messages.iter() {
        let envelope = message.envelope().expect("message did not have an envelope!");
        let from_addr = addresses_to_string(envelope.from.as_ref().expect("message envelope did not have a from address!"));
        // println!("from: {}", from_addr);
        let subject = match envelope.subject {
            Some(ref x) => std::str::from_utf8(x).expect("Message Subject was not valid utf-8").to_string(),
            None => "".to_string(),
        };
        // let subject = std::str::from_utf8(envelope.subject.as_ref().expect("Envelope did not have a subject!"))
        //     .expect("Message Subject was not valid utf-8").to_string();
        // println!("subject: {}", subject);

        for checker in &checkers {
            if match_mail(checker, &from_addr, &subject) {
                trashables.push(message.message.to_string())
            }
        }
    }

    if trashables.len() > 0 {
        if noop {
            println!("In no-op mode, doing nothing with {} mails.", trashables.len());
        } else {
            println!("Moving {} mails to trash.", trashables.len());
            let sessout = imap_session.mv(trashables.join(","), "[Gmail]/Trash").expect("Move to trash failed!");
            println!("Moved to trash: {:?}", sessout);
        }
    } else {
        println!("No mails to move.")
    }

}

// fn fetch_inbox_top() -> imap::error::Result<Option<String>> {
// 
//     // we want to fetch the first email in the INBOX mailbox
//     imap_session.select("INBOX")?;
// 
//     // FIXME: Dynamic date!!
//     let seqs = imap_session.search("SINCE 27-Feb-2022")
//         .expect("Could not search for recent messages!");
// 
//     // let seqs_list = seqs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
//     let seqs_list = seqs.iter().fold("".to_string(), |acc, x| {
//         if acc == "" {
//             x.to_string()
//         } else {
//             format!("{},{}", acc, x)
//         }
//     });
//     // let seqs_list = seqs.iter().fold("".to_string(), |acc, x| format!("{},{}", acc, x));
//     let messages = imap_session.fetch(seqs_list, "ENVELOPE")
//         .expect("Couldn't fetch messages!");
// 
//     let mut trashables = Vec::new();
// 
//     for message in messages.iter() {
//         let envelope = message.envelope().expect("message did not have an envelope!");
//         let from_addr = addresses_to_string(envelope.from.as_ref().expect("message envelope did not have a from address!"));
//         let subject = std::str::from_utf8(envelope.subject.as_ref().expect("Envelope did not have a subject!"))
//             .expect("Message Subject was not valid utf-8");
// 
//         if (from_addr.ends_with("<spdrata@digitalkingdom.org>") && subject.starts_with("Cron"))||
//             (from_addr.ends_with("<spcorpus@stodi.digitalkingdom.org>") && subject.starts_with("Cron"))||
//             (from_addr.ends_with("<spjbovlaste@digitalkingdom.org>") && subject.starts_with("Cron"))||
//             (from_addr.ends_with("<root@stodi.digitalkingdom.org>") && subject.starts_with("Cron"))||
//             (from_addr.ends_with("<sphaproxy@digitalkingdom.org>") && subject.starts_with("Cron"))||
//             (from_addr.ends_with("<sptwjbo@stodi.digitalkingdom.org>") && subject.starts_with("Cron"))
//         {
//             println!("{:?}", from_addr);
//             println!("{:?}", subject);
// 
//             trashables.push(message.message.to_string())
//         }
//     }
// 
//     let sessout = imap_session.mv(trashables.join(","), "[Gmail]/Trash").expect("Move to trash failed!");
//     println!("Moved to trash: {:?}", sessout);
// 
// //            imap_session.mv(message.message.to_string(), "[Gmail]/Trash").expect("Move to trash failed!");
//  //           thread::sleep(time::Duration::from_secs(1));
// 
//   //  }
// 
// //     let message = if let Some(m) = messages.iter().next() {
// //         m
// //     } else {
// //         return Ok(None);
// //     };
// // 
// //     // extract the message's body
// //     let header = message.header().expect("message did not have a header!");
// //     let header = std::str::from_utf8(header)
// //         .expect("message was not valid utf-8")
// //         .to_string();
// // 
// //     // be nice to the server and log out
// //     imap_session.logout()?;
// // 
// //     Ok(Some(header))
//     Ok(Some("OK".to_string()))
// }
