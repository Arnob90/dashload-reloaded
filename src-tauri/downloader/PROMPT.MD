The internal signatures of `dashload-downloader` have been refactored (state machine introduced, new fields added to DownloadData).

Task:
1. Check out tests/, and run them one by one.
2. Inspect the compilation and test errors.
3. Update the outdated test cases in `tests/` to match the new public API (do not weaken any assertions). If the behaviour has deviated from the tests, DOCUMENT IT in the test case and update the assertions accordingly.
ALSO, let me know.
4. Run `cargo test` until all tests pass.
5. Commit the fixes.
6. Call `@reviewer` to audit the diff and confirm zero regressions.
7. Do not commit anything. Keep your changes in the working tree
