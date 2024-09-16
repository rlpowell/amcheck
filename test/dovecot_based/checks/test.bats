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

@test "checker test" {
  _launch_imapd "$thistestdir/initial_mail/" "$testingdir/dovecot_based/" "$mail_tempdir/" "$config_tempdir/" 3>&-

  # Update the mail date on one mail to make it recent
  newdate="$(date -d yesterday -R)"
  sed -r -i "s/Fri, 01 Jan 1999.*/$newdate/" "$mail_tempdir/amcheck_storage/new/1725336071.108351_3.2b60b02dea90"

  run cargo run check
  assert_output --partial 'message="Deleting 2 mails." noop=false checker_sets_count=3 name="Puppet Runs OK" action=Delete'
  assert_output --regexp 'level=warn.*CHECK FAILED for check ..Puppet Runs OK.. for 1 mails'
  assert_output --regexp 'message=.Check ..Puppet Runs At Least Once A Day.. passed with 1 mails found being more than 1'
  assert_output --regexp 'message="Deleting 1 mails." noop=false checker_sets_count=3 name="delete rsync_backup_wrapper mail"'
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