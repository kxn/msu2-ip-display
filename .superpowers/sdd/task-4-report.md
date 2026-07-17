Task 4 report

What I implemented
- Added `pub mod ip_detect;` to `host-usb/src/lib.rs` while preserving the existing exports and tests.
- Created `host-usb/src/ip_detect.rs` with the Task 4 data types: `NetworkSnapshot`, `AddressCandidate`, `Route`, `SelectionConfig`, and `Selection`.
- Implemented pure IPv4 selection logic in `select_ipv4(snapshot, config)` with the requested precedence:
  - fixed interface override
  - default route interface preference
  - single normal IPv4 fallback
  - dynamic-vs-static disambiguation
  - link-local-only failure handling
  - virtual interface exclusion in fallback selection
- Added unit tests covering the six cases from the brief.

TDD RED
- Command: `cd host-usb; cargo test ip_detect::tests -- --nocapture`
- Result: 5 tests failed, 1 passed.
- Failure summary: every behavior test still returned `Selection::Pending` from the stub, which was the expected red state.

GREEN / final verification
- Command: `cd host-usb; cargo test ip_detect::tests -- --nocapture`
- Result: 6 passed, 0 failed.
- Command: `cd host-usb; cargo test`
- Result: 18 passed, 0 failed.

Commit hash
- `253dc95`

Self-review notes / concerns
- The implementation is intentionally narrow and purely local; it does not add any syscall or platform collection code.
- Virtual-interface filtering is name-prefix based, matching the brief. If future platform data expands beyond those names, this will need a follow-on task rather than a broadening here.

Fix after review
- Finding: fallback link-local classification was still treating virtual interfaces like `docker0` as `FailureCandidate`.
- RED: `cd host-usb; cargo test ip_detect::tests -- --nocapture`
  - Result after adding `virtual_link_local_only_is_pending_for_fallback`: 6 passed, 1 failed.
  - Failure was `Selection::FailureCandidate` vs expected `Selection::Pending`.
- GREEN: narrowed the fallback link-local failure check to non-virtual interfaces only.
  - Re-ran `cd host-usb; cargo test ip_detect::tests -- --nocapture`
  - Result: 7 passed, 0 failed.
- Full verification: `cd host-usb; cargo test`
  - Result: 19 passed, 0 failed.
