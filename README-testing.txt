Run all the tests like so in the container:

$ ./test/bats/bin/bats -r test/dovecot_based/

Relies on sudo to launch dovecot-imapd

Debug run:

$ RUST_LOG=debug cargo run

Specifying a config file:

$ export AMCHECK_CONFIG_FILE=test/dovecot_based/simple/settings.json5

Showing the output:

$ TEST_OVERRIDE_RUST_LOG=trace ./test/bats/bin/bats --verbose-run --show-output-of-passing-tests -r test/dovecot_based/checks/ 2>&1 | less
