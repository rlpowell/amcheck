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
    echo "Second argument is the directory with the config template and cert files."
    exit 1
  fi

  if [[ ! -d $config_tempdir ]]
  then
    echo "Second argument is the directory with the config template and cert files."
    exit 1
  fi

  cp -pr "$initial/"* "$mail_tempdir/"
  mkdir "$mail_tempdir/index" "$mail_tempdir/control" "$mail_tempdir/tmp"

  sed "s;##LOCATION##;$mail_tempdir;g" "$template_dir/dovecot.conf.template" >"$config_tempdir/dovecot.conf"
  cp "$template_dir/cert.pem" "$config_tempdir"
  cp "$template_dir/key.pem" "$config_tempdir"

  cd "$config_tempdir"

  sudo /usr/sbin/dovecot -F -c "$config_tempdir/dovecot.conf" >/tmp/bats-dovecot.out 2>&1 &
  echo $! > /tmp/bats-dovecot.pid
  sleep 3

  cd "$startdir"
}
