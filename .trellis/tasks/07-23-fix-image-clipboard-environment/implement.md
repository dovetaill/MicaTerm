# Implementation Plan

1. Add a tested Windows image-source selector that recognizes registered PNG,
   bitmap-convertible DIB/DIBV5 formats, and exactly one supported file.
2. Add bounded `HGLOBAL` copying for registered PNG and route it through the
   existing constrained image encoder.
3. Change the bitmap path to request a synthesized `CF_BITMAP` when DIB or
   DIBV5 is available, retaining metadata preflight before byte allocation.
4. Add focused clipboard tests for source priority, PNG encoding, and limits.
5. Update the backend quality specification with the real Windows clipboard
   format contract and `TERM_PROGRAM` best-effort decision.
6. Run formatting, focused tests, SFTP/session regressions, full tests as
   practical, Clippy, diff checks, and Windows GNU all-target compilation.

## Rollback

The change is local to clipboard source recognition. Reverting the feature
commit restores the prior `CF_BITMAP`/file-list-only behavior without changing
remote SFTP state or terminal protocol state.
