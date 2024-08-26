setup() {
  load "$(pwd)/test/test_helper/common-setup"
  _common_setup
  load "$testingdir/test_helper/launch-imapd"
  load "$testingdir/test_helper/kill-imapd"

  export RUST_LOG=info
  export AMCHECK_ENVIRONMENT=test
  export AMCHECK_CONFIG_FILE="$thistestdir/settings.toml"

  mail_tempdir="$(temp_make)"
  config_tempdir="$(temp_make)"
}

@test "single checker test" {
  _launch_imapd "$thistestdir/initial_mail/" "$testingdir/dovecot_based/" "$mail_tempdir/" "$config_tempdir/" 3>&-

  run cargo run move
  assert_output --partial "Moving 3 mails to storage."
  refute_output "error"
  refute_output "warning"
  assert_success

  run $testingdir/dovecot_based/maildir_diff.sh "$mail_tempdir" "$thistestdir/target_mail/"
  assert_success
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
