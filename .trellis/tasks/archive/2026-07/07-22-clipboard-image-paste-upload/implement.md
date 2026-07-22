# Implementation Plan

1. Add direct Windows-only `clipboard-win` and `image` dependencies.
2. Add tests for the payload-selection policy and POSIX path quoting.
3. Implement Windows bitmap/file-list extraction and bounded PNG encoding behind
   `cfg(target_os = "windows")`.
4. Extend the SFTP runtime contract with bounded byte upload and secure/exclusive
   remote cache creation.
5. Integrate the async upload branch into the shared workspace paste callback while
   leaving the existing text branch intact.
6. Add cleanup and backend contract tests.
7. Run formatting, targeted tests, Linux compilation, and the relevant regression
   suite before completing the child task.
