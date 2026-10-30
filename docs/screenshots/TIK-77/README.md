# Mission Control v1 evidence

Dark fixture captures, with bundled fonts loaded:

| Capture | Route / viewport |
| --- | --- |
| before-mission-desktop.png | Base `b8e85d1`, `/design/home`, 1440 × 1000 |
| before-mission-mobile.png | Base `b8e85d1`, `/design/home`, 390 × 844, full page |
| before-mission-mobile-sidebar.png | Same base/mobile, sidebar open, no Mission Control link |
| after-mission-desktop.png | `/design/mission`, 1440 × 1000, full page |
| after-mission-mobile.png | `/design/mission`, 390 × 844, full page |
| after-mission-mobile-sidebar.png | Mission Control selected in mobile sidebar |
| after-mission-component.png | `/design/c/mission`, tightly cropped, 2× DPR |

After captures inherit the Geist default from the separately merged TIK-83;
the before captures retain the original Inter baseline. Mission Control uses
the shared font tokens.

Mission Control did not exist at the base revision; the before captures preserve
the real Home/sidebar. The after routes render the same Mission page using only
sample data from `fixtures.rs`. All seven sections are populated, with CI pass,
pending and fail, auto-merge, a stale source and both build kinds. Fixture times
are relative to capture time so ready-to-test items remain within 48 hours.

Reproduce with `cargo xtask component-shot mission --output docs/screenshots/TIK-77`.
Use `--before-assets <base-dist>` for authentic baseline assets. The command
checks horizontal overflow at desktop/mobile and fixture approval removal.
`cargo xtask mission-check` separately checks the real local Worker approval,
comment-failure compensation, focus refresh, retained data on refresh failure
and mobile overflow. No production fixtures are written.

Impeccable critique used two independent assessments: design review scored
33/40 and recommended shipping v1 after functional checks; the source detector
returned zero findings (Rust markup coverage is limited). Desktop/mobile and
component captures showed no clipping or missing sections. The polish pass
resolved premature empty confirmation during approval with live pending feedback
and heading focus, and checked direct-entry sidebar loading. The documentation
pass confirmed alignment with DESIGN.md; broader visual refinement stays TIK-81.

Local release verification used a temporary distinct web binary name and a
workspace compiler wrapper to prevent concurrent worktrees from replacing
shared Cargo/wasm-bindgen artifacts. The configured Cargo target directory was
preserved, and the tracked manifest/index configuration was restored afterward.
CI uses the normal checked-in production build configuration.
