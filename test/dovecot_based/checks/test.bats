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

  # Force a build so the lines below line up correctly, without extra compilation stuff
  run cargo build
  assert_success

  run run_with_fmt

  refute_output --partial "failed"
  refute_output --partial "error"
  refute_output --partial "warning"

  # We are very specific with the exact layout of the output because sometimes
  # changes have accidentally run too many checks
  assert_line --index 4 --regexp 'WARN.*CHECK FAILED for check .Artificial BodyCheckAny. for 4 mails'
  assert_line --index 5 --partial "CHECK FAILED DETAILS for check 'Artificial BodyCheckAny': mail from '\"(Cron Daemon)\" <root@mysite.org>' with subject 'Cron <root@bbox> /opt/puppetlabs/bin/puppet agent -t 2>&1 | grep -Ev '^.......(notice): '' and date '2023-01-01 12:16:25.0 -08:00:00'"
  assert_line --index 6 --partial "CHECK FAILED DETAILS for check 'Artificial BodyCheckAny': mail from '\"(Cron Daemon)\" <root@mysite.org>' with subject 'Cron <root@bbox> /opt/puppetlabs/bin/puppet agent -t 2>&1 | grep -Ev '^.......(notice): '' and date '1999-01-01 12:16:25.0 -08:00:00'"
  assert_line --index 7 --partial "CHECK FAILED DETAILS for check 'Artificial BodyCheckAny': mail from '\"\" <not.a.puppet.match@test.org>' with subject 'Does Not Match For Puppet' and date '1999-01-01 12:16:25.0 -08:00:00'"
  assert_line --index 8 --partial "CHECK FAILED DETAILS for check 'Artificial BodyCheckAny': mail from '\"root\" <root@mysite.org>' with subject 'Cron <root@abox> /usr/local/bin/rsync_backup_wrapper --config /usr/local/etc/rsync_backup_drive_config.yaml' and date '2023-12-01 14:31:01.0 -08:00:00'"

  assert_line --index 9 --regexp 'WARN.*CHECK FAILED for check .Artificial BodyCheckAll. for 1 mails'
  assert_line --index 10 --partial "CHECK FAILED DETAILS for check 'Artificial BodyCheckAll': mail from '\"(Cron Daemon)\" <root@mysite.org>' with subject 'Cron <root@bbox> /opt/puppetlabs/bin/puppet agent -t 2>&1 | grep -Ev '^.......(notice): '' and date '1999-01-01 12:16:25.0 -08:00:00'"

  assert_line --index 11 --regexp 'WARN.*CHECK FAILED for check .Artificial BodyCheckRegex. for 1 mails'
  assert_line --index 12 --partial "CHECK FAILED DETAILS for check 'Artificial BodyCheckRegex': mail from '\"(Cron Daemon)\" <root@mysite.org>' with subject 'Cron <root@bbox> /opt/puppetlabs/bin/puppet agent -t 2>&1 | grep -Ev '^.......(notice): '' and date '2023-01-01 12:16:25.0 -08:00:00'"

  assert_line --index 13 --regexp 'WARN.*CHECK FAILED for check .Check for shallow count 0 failure. for 0 mails.*tree_head_type.*"CountCheck 1"'

  assert_line --index 14 --regexp 'WARN.*CHECK FAILED for check .Check for deep count 0 failure. for 0 mails.*tree_head_type.*"CountCheck 1"'

  assert_line --index 15 --regexp 'amcheck.*Deleting 2 mails.*name.*"Puppet Runs OK And At Least Once In The Past Day".*tree_head_type.*"Action Delete".*tree_head_type.*"DateCheck 1".*tree_head_type.*"BodyCheckAny ...Notice: Applied catalog in'
  assert_line --index 16 --regexp 'amcheck.*Check .Puppet Runs OK And At Least Once In The Past Day. passed with 1 mails'
  assert_line --index 17 --regexp 'WARN.*CHECK FAILED for check .Puppet Runs OK And At Least Once In The Past Day. for 1 mails'
  assert_line --index 18 --partial "CHECK FAILED DETAILS for check 'Puppet Runs OK And At Least Once In The Past Day': mail from '\"(Cron Daemon)\" <root@mysite.org>' with subject 'Cron <root@bbox> /opt/puppetlabs/bin/puppet agent -t 2>&1 | grep -Ev '^.......(notice): '' and date '2023-01-01 12:16:25.0 -08:00:00'"

  assert_line --index 19 --regexp 'amcheck.*Deleting 1 mails.*name.*"delete rsync_backup_wrapper mail".*tree_head_type.*"Action Delete".*tree_head_type.*"MatchCheck .."'

  assert [ ${#lines[@]} -eq 20 ]

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
