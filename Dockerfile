FROM neovim_rust

ARG USERNAME
ARG UID
ARG GID

RUN apt-get update
# dovecot for imap testing
# procmail for maildir_diff.sh
# sudo for running dovecot from our test scripts
# mutt for manual testing
RUN apt-get install -y dovecot-imapd procmail sudo mutt

RUN echo "${USERNAME}    ALL=(ALL)    NOPASSWD: ALL" > /etc/sudoers.d/${USERNAME}
