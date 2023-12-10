#!/bin/bash

_kill_imapd() {
  if [[ -f /tmp/bats-dovecot.pid ]]
  then
    kill "$(< /tmp/bats-dovecot.pid)" || true
    rm -f /tmp/bats-dovecot.pid || true
  else
    echo "No dovecot PID"
  fi
}
