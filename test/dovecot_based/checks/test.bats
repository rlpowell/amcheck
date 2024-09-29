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

run_with_fmt() {
  cargo run check | sed 's/^\s*//' | fmt -w 1234
}

@test "checker test" {
  _launch_imapd "$thistestdir/initial_mail/" "$testingdir/dovecot_based/" "$mail_tempdir/" "$config_tempdir/" 3>&-

  # Update the mail date on one mail to make it recent
  newdate="$(date -d '1 hour ago' -R)"
  sed -r -i "s/Fri, 01 Jan 1999.*/$newdate/" "$mail_tempdir/amcheck_storage/new/1725336071.108351_3.2b60b02dea90"

  run run_with_fmt
  assert_output --regexp 'amcheck.*Deleting 2 mails.*name.*"Puppet Runs OK And At Least Once In The Past Day".*tree_head_type.*"Action Delete".*tree_head_type.*"DateCheck 1".*tree_head_type.*"BodyCheck Notice: Applied catalog in"'
  assert_output --regexp 'WARN.*CHECK FAILED for check .Puppet Runs OK And At Least Once In The Past Day. for 1 mails'
  assert_output --regexp 'amcheck.*Check .Puppet Runs OK And At Least Once In The Past Day. passed with 1 mails'
  assert_output --regexp 'amcheck.*Deleting 1 mails.*name.*"delete rsync_backup_wrapper mail".*tree_head_type.*"Action Delete".*tree_head_type.*"MatchCheck .."'
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
