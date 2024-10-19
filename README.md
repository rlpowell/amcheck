# What Is This?

amcheck is a system for automating checks on mails, typically mails produced by other automations.  Something like a very verbose procmail but (1) for IMAP and (2) with a focus on reporting on emails that look like something went wrong.

# Wait, What?

Look at [the puppet handler description below](#the-puppet-handler-example), which is talking about [the puppet block in the example settings file](settings/prod.json5.example#L19).  That clause handles an automated email I get from Puppet, a configuration management system.  If this still doesn't make sense, this project is probably not for you.

# Basic Usage

amcheck has two modes; `move` and `check`.  Give those words as the first argument when you run it; it takes no other arguments, everything else is done by config file.

In `move` mode, amcheck moves all the mails you've say you want it to be in charge of from you (by default) `INBOX` imap folder to your (by default) `amcheck_storage` IMAP folder.  This is intended to be run many times a day, to keep your inbox clean of automated emails.  It typically runs quickly (20 seconds or so).

In `check` mode, amcheck checks all the mails in the storage folder against the rules you've specified, and calls out any that don't look right, or calls out if it hasn't seen a mail you've told it to expect.  For me, with 96 rules, this typically takes about 5 minutes.  This scales with number of rules and complexity of them, and scales sharply with usage of email body regex checks.

# Logging

By default amcheck logs at INFO level, but once you have your `check` phase working, it can be very useful to set it to `warn`, which you can do with `RUST_LOG=warn`.

# Config File Location

Defaults to `./settings/prod.json5`; can be overridden with the environment variable `AMCHECK_CONFIG_FILE`.  You can run in the test environment by setting the environment variable `AMCHECK_ENVIRONMENT` to `test`, all that does is change the default config file to `./settings/test.json5`, so probably not really that useful to you.

# Config File Structure And Use

The rest of this document assumes you're looking at [the example settings file](settings/prod.json5.example), although I'll note that my config doesn't use json5 directly and is basically the same in structure as `settings/prod.pkl.example` except much, much longer; it was too repetitive so I turned to [pkl](https://pkl-lang.org/main/current/index.html) for configuration expansion.

The imap settings should be obvious.  `days_back` is how old amcheck should search back in your inbox for mail to filter.  Everything else is in the list of handlers.

The first handler, the one for a Puppet run, shows the basic structure well.

In the `move` phase, the various filters are run against all your inbox mail, and any matching mail is moved to your storage folder.

In the `check` phase, all the filters are run again, and then the checker_tree is walked until an `Action` or `Stop` is reached.

## The Puppet Handler Example

So the puppet handler says:

- take all mails that are from root
    - and are Cron mails
    - and are puppet emails
- and if they succeeded (the "Notice: Applied catalog in" string only appears in successful puppet runs)
    - delete any mails older than 1 day
    - check that the count of mails younger than 1 day is at least one
        - if it isn't, alert
- any that didn't succeed, alert

At each step, the `empty_ok` field says which branch to follow if, in fact, no mails are left.

# Filters

A filter run starts with all the mail in your inbox less than `days_back` old.

Each filter is then applied in turn on whatever mails remain at that point.

`Match` keeps any mails that match its criteria.

`UnMatch` drops any mails that match its criteria.

## Subject

This matches the email subject against the given regular expression.

## From

This matches the email from address (just for clarity: the 'From:' header not the 'From ' header) against the given regular expression.

## No Body Filters

Body filters are slow, and I didn't see the need, but it's certainly possible; bug me if you really want this feature.

## Empty Filter List

As a special case, an empty filter list is ignored in the move phase, but in the check phase, it matches all mails.  This allows rules like "if a mail older than 10 days is in storage, we must not be handling it; alert on it".

# Matchers

Matchers (except Action and Stop) all have the same basic structure: on the current set of emails, perform a test, and all the emails get routed to the part of the tree that matches how they responded to the test.  For most matchers this is the `matched` or `not_matched` arms.

The branch named by `empty_ok` is followed whether any mails matched that branch or not.  The primary purpose here is to make sure that `CountCheck`s are evaluated even if we excluded all emails earlier on in the tree.  `CountCheck` does not have an `empty_ok`, since it has an explicit branch for that case.

## Stop

Just what it says; do nothing, just stop.

## Action

Do something with any mails that reach this point; do not continue.  Available actions are Alert, Success (just produces a log line),

## MatchCheck

Takes a list of filters in the `matchers` field; basically just a way to re-use the filters in the checker_tree.

## DateCheck

Takes a number of days ago in the `days` field, and splits mail into `older_than` and `younger_than`.

## CountCheck

Counts the number of mails that reach it, and depending on the value of that
compared to the `count` field, sends all the mails it got down the
`greater_than`, `less_than` or `equals` branches.

## BodyCheckAny

Takes a list of strings in `strings`, and checks the mails against any of them, into the `matched` and `not_matched` trees.

Only takes literal strings, and in fact depending on the IMAP server, might only check whole words (i.e. `err` in the strings will not match `error` in the mails).  Furthermore, on most (all?) IMAP servers, the matching is case-insensitive.

## BodyCheckAll

Same as `BodyCheckAny`, but all the strings must match.

## BodyCheckRegex

Takes a single `regex` and matches that against the mails as usual.

Unlike `BodyCheckAny` and `BodyCheckAll`, this does not use IMAP facilities to do the matching; it must retrieve the entirety of every mail that reaches it to do the matching.  As a result, it is quite slow, and this slowness ramps up substantially with the number of mails.  You should try to make sure that no more than a couple of hundred emails reach this check at any one time.
