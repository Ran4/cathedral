#!/bin/bash
# Run from repository root. Optional first argument selects one test module.
set -u
evidence=features/2026-09-04_gpt-6_astra_xhigh_suggestions_for_things_to_make_the_game_more_fun_to_play/implementation/plan/evidence/m3_http/owner
printf 'START '
/usr/bin/date --utc --iso-8601=seconds
printf 'HEAD '
/usr/bin/git rev-parse HEAD
printf 'Environment: login:false; default dev/test profile, repository target/, default /home/ran/.cargo; compiler flags and wrappers unset\n'
for variable in CARGO_HOME CARGO_TARGET_DIR RUSTC RUSTDOC RUSTFLAGS CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_RUSTFLAGS RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER LDFLAGS; do
    if [[ -v $variable ]]; then
        printf 'Unexpected environment override: %s\n' "$variable"
        exit 2
    fi
done
/home/ran/.cargo/bin/rustc -Vv
/home/ran/.cargo/bin/cargo -V
/usr/bin/sha256sum --check "$evidence/source-sha256.txt" || exit 2
command=(/usr/bin/nice -n 15 /home/ran/.cargo/bin/cargo test --locked --offline -j 1 -p cathedral-backends --lib)
if [[ $# -gt 0 ]]; then command+=("$1"); fi
command+=(-- --test-threads=1)
printf 'COMMAND '
printf '%q ' "${command[@]}"
printf '\n'
"${command[@]}"
test_status=$?
/usr/bin/sha256sum --check "$evidence/source-sha256.txt"
identity_status=$?
/usr/bin/find target/debug/deps -maxdepth 1 -type f -name 'cathedral_backends-*' -executable -exec /usr/bin/sha256sum '{}' \;
printf 'END '
/usr/bin/date --utc --iso-8601=seconds
printf 'TEST_EXIT %s\nIDENTITY_EXIT %s\n' "$test_status" "$identity_status"
if [[ $test_status -ne 0 ]]; then exit "$test_status"; fi
exit "$identity_status"
