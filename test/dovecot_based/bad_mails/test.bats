setup() {
  load "$(pwd)/test/test_helper/common-setup"
  _common_setup
  load "$testingdir/test_helper/launch-imapd"
  load "$testingdir/test_helper/kill-imapd"

  export RUST_LOG=${TEST_OVERRIDE_RUST_LOG:-info}
  export AMCHECK_ENVIRONMENT=test
  export AMCHECK_CONFIG_FILE="$thistestdir/settings.json5"

  mail_tempdir="$(temp_make)"
  config_tempdir="$(temp_make)"
}

@test "bad mails test" {
  _launch_imapd "$thistestdir/initial_mail/" "$testingdir/dovecot_based/" "$mail_tempdir/" "$config_tempdir/" 3>&-

  run cargo run move
  assert_output --regexp 'level=warn.*Mail format error: No addresses found'
  assert_output --regexp 'level=warn.*Mail format error: Couldn..t convert a name in an Address to UTF-8'
  assert_output --regexp 'level=warn.*Mail format error: Couldn..t convert a local part ...mailbox... in an Address to UTF-8'
  assert_output --regexp 'level=warn.*Mail format error: Couldn..t convert a host in an Address to UTF-8'
  assert_output --regexp 'level=warn.*Mail format error: Message Subject was not valid utf-8'
  assert_output --regexp 'level=warn.*Mail format error: No Subject found'
  assert_output --regexp 'level=warn.*Mail format error: Message Date was not valid utf-8'
  assert_output --regexp 'level=warn.*Mail format error: No Date found'
  assert_success

  # Put bad mail in place
  cp test/dovecot_based/bad_mails/bad_body_1.abox "$mail_tempdir/amcheck_storage/new/1702449998.287739_5.abox"

  run cargo run check
  assert_output --partial 'Message body was not valid utf-8'
  assert_failure
}

teardown() {
  # We do this here in case the tests fails early
  _kill_imapd

  # Note the existence of BATSLIB_TEMP_PRESERVE and
  # BATSLIB_TEMP_PRESERVE_ON_FAILURE ; see 
  # https://github.com/ztombol/bats-file?tab=readme-ov-file#working-with-temporary-directories
  temp_del "$config_tempdir"
  # The "yes" is in answer to "rm: remove write-protected regular empty file?"
  yes | temp_del "$mail_tempdir"
}
