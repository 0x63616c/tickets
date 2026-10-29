# TIK-38 review evidence

Baseline: `63107b4275807a3a54956b8708921d1153dd20fc`. Before PNGs use the unchanged built assets saved before
implementation, served locally with fixture data. After PNGs use `/design/c/*`.
All component captures are dark, font-loaded, cropped at 2× DPR.

| Component | Before | After |
|---|---|---|
| Select | before-select.png | after-select.png |
| Menu | before-menu.png | after-menu.png |
| Multi-select labels | before-combobox.png | after-combobox.png |
| Modal | before-modal.png | after-modal.png |
| ASCII loader | before-loader.png | after-loader.png |
| Skeleton | before-skeletons.png | after-skeletons.png |
| Project icons | before-icons.png | after-icons.png |
| Busy buttons | before-busy-button.png | after-busy-button.png |

The baseline had plain loading text and a disabled submit button, captured by
holding local requests. Chrome page screenshots cannot include an operating-system
native select menu, so before-select shows its focused trigger; after-select shows
the complete open listbox. Before-menu shows the previous ad-hoc label menu.

`after-skeletons-list`, `after-skeletons-detail` and `after-skeletons-projects`
cover the other skeleton layouts. Full-page fixture checks cover desktop/mobile
integration. No production data, tokens or personal information are used.

Reproduce with `cargo xtask build`, then
`CHROME_BIN=/usr/bin/google-chrome cargo xtask component-shot all`.
Use `--variant list|detail|projects` for skeletons and `--output <dir>` to redirect.
Use `--before-assets <dist-directory>` for an actual previous build.

This environment's managed Chrome proxy required a process-local bwrap policy
overlay for loopback access; checks used the same `/usr/bin/google-chrome` binary.
The override is temporary and does not change the shared machine policy.
