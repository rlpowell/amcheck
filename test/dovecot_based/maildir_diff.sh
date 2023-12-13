#!/bin/bash

# Error trapping from https://gist.github.com/oldratlee/902ad9a398affca37bfcfab64612e7d1
__error_trapper() {
  local parent_lineno="$1"
  local code="$2"
  local commands="$3"
  echo "error exit status $code, at file $0 on or near line $parent_lineno: $commands"
}
trap '__error_trapper "${LINENO}/${BASH_LINENO}" "$?" "$BASH_COMMAND"' ERR

set -euE -o pipefail

# You can't just diff -r two maildir folders because all the files
# will be different; this script uses formail to generate a unique
# line for each mail, including which folder each one is in, then we
# compare those lines.

testdir="$1"
targetdir="$2"

if [[ ! -d $testdir ]]
then
  echo "First argument is the mail directory for your test run."
  exit 1
fi

if [[ ! -d $targetdir ]]
then
  echo "Second argument is the target state mail directory."
  exit 1
fi

cd "$testdir"
# Doing the dovecot internal stuff cleanup here means we don't have
# to account for it in the diffs, *and* temp_del in the bats test is
# much more likely to actually work
rm -rf index/ control/ tmp/
while IFS= read -r -d '' file
do
  dir="$(echo "$file" | sed -r 's;^./([^/]*)/.*;\1;')"
  headers="$(formail -c -X Message-Id -X From: -X To -X Subject <"$file" | tr '\n' ' ')"
  echo "$dir $headers"
done < <(find . -type f -print0 || true) | sort -k2 >/tmp/testdir.$$

cd "$targetdir"
while IFS= read -r -d '' file
do
  dir="$(echo "$file" | sed -r 's;^./([^/]*)/.*;\1;')"
  headers="$(formail -c -X Message-Id -X From: -X To -X Subject <"$file" | tr '\n' ' ')"
  echo "$dir $headers"
done < <(find . -type f -print0 || true) | sort -k2 >/tmp/targetdir.$$

retval=0

wc -l /tmp/testdir.$$ /tmp/targetdir.$$
if ! diff /tmp/testdir.$$ /tmp/targetdir.$$ >/dev/null 2>&1
then
  echo "Diff output:"
  git diff --word-diff=color /tmp/testdir.$$ /tmp/targetdir.$$
  echo "Directory counts:"
  for name in $(find "$testdir" "$targetdir" -type d)
  do
    echo "*** $name"
    ls -l $name | wc -l
  done
  retval=1
fi
# rm /tmp/testdir.$$ /tmp/targetdir.$$

exit "$retval"
