FROM docker.io/library/rust:1.74-bookworm

ARG USERNAME

RUN apt-get update
# dovecot for imap testing
# procmail for maildir_diff.sh
# sudo for running dovecot from our test scripts
# other bits for manual testing
RUN apt-get install -y dovecot-imapd procmail sudo mutt vim less

RUN echo "${USERNAME}    ALL=(ALL)    NOPASSWD: ALL" > /etc/sudoers.d/${USERNAME}

COPY Cargo.lock /tmp/cargo-bootstrap/
COPY Cargo.toml /tmp/cargo-bootstrap/

WORKDIR /tmp/cargo-bootstrap/

RUN mkdir src
RUN touch src/main.rs

RUN cargo fetch
