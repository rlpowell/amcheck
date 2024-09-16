// Wrapping the Uid and Seq types from the imap library in non-equivalent types.

use std::collections::HashSet;
use std::fmt;
use std::io::{Read, Write};

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub struct Seq(u32);

impl fmt::Display for Seq {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub struct Uid(u32);

impl fmt::Display for Uid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Seq> for u32 {
    fn from(item: Seq) -> u32 {
        item.0
    }
}

impl From<Uid> for u32 {
    fn from(item: Uid) -> u32 {
        item.0
    }
}

impl From<u32> for Seq {
    fn from(item: u32) -> Seq {
        Seq(item)
    }
}

impl From<u32> for Uid {
    fn from(item: u32) -> Uid {
        Uid(item)
    }
}

pub fn my_search<T: Read + Write>(
    session: &mut imap::Session<T>,
    query: impl AsRef<str>,
) -> imap::error::Result<HashSet<Seq>> {
    let orig = session.search(query);
    match orig {
        Ok(orig) => Ok(HashSet::from_iter(orig.iter().map(|x| Seq::from(*x)))),
        Err(x) => Err(x),
    }
}

pub fn my_uid_search<T: Read + Write>(
    session: &mut imap::Session<T>,
    query: impl AsRef<str>,
) -> imap::error::Result<HashSet<Uid>> {
    let orig = session.uid_search(query);
    match orig {
        Ok(orig) => Ok(HashSet::from_iter(orig.iter().map(|x| Uid::from(*x)))),
        Err(x) => Err(x),
    }
}
