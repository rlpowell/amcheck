#!/bin/bash

_launch_imapd() {
  startdir="$(pwd)"

  initial="$1"
  template_dir="$2"
  mail_tempdir="$3"
  config_tempdir="$4"

  if [[ ! -d $initial ]]
  then
    echo "First argument is the initial state mail directory for your test run."
    exit 1
  fi

  if [[ ! -d $template_dir ]]
  then
    echo "Second argument is the directory with the config template and cert files."
    exit 1
  fi

  if [[ ! -d $mail_tempdir ]]
  then
    echo "Third argument is the directory where the actual mail goes for dovecot to read."
    exit 1
  fi

  if [[ ! -d $config_tempdir ]]
  then
    echo "Fourth argument is the directory to put the config file in for dovecot to use."
    exit 1
  fi

  cp -pr "$initial/"* "$mail_tempdir/"
  mkdir "$mail_tempdir/index" "$mail_tempdir/control" "$mail_tempdir/tmp"

  sed "s;##LOCATION##;$mail_tempdir;g" "$template_dir/dovecot.conf.template" >"$config_tempdir/dovecot.conf"
  cp "$template_dir/cert.pem" "$config_tempdir"
  cp "$template_dir/key.pem" "$config_tempdir"

  cd "$config_tempdir"

  # Set up UID files for dovecot; this is so the sequence numbers and the uid
  # numbers don't match, so our testing is more robust
  for folder in $(\ls "$initial/")
  do
    mkdir -p "$mail_tempdir/control/$folder"
    sse="$(date +%s)"
    echo "3 V$sse N1234 G$(cat /proc/sys/kernel/random/uuid | tr -d '\n-')" > "$mail_tempdir/control/$folder/dovecot-uidlist"
  done

  sudo /usr/sbin/dovecot -F -c "$config_tempdir/dovecot.conf" >/tmp/bats-dovecot.out 2>&1 &
  echo $! > /tmp/bats-dovecot.pid
  sleep 3

  ps -p "$(cat /tmp/bats-dovecot.pid)" >/dev/null || {
	  echo "Couldn't start dovecot."
	  false
  }

  cd "$startdir"
}
