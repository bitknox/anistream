# Codemods for the engine extraction

These scripts rewrite code that uses main's names so it uses this branch's. Run all five after
each merge of main, in this order, from the repository root or anywhere inside it:

1. `notice.sh`: the toast that crosses into the interface becomes a `Notice`, and its kind a
   `NoticeKind`.
2. `rename.sh`: at the boundary, `Task` becomes `Command` and `Update` becomes `Event`.
3. `model_paths.py`: the types and wording that moved into anistream-model are named by their
   paths there.
4. `crate_paths.sh`: the old library's paths become anistream-engine's; tty, focus and the
   updater are named as the binary's `tui::terminal`, `tui::focus` and `cli::update`; and
   anistream-ui becomes anistream-tui.
5. `test_accessors.py`: tests read and set the App's cursors and text buffers through accessors,
   `a.cursor()` and `a.text(Field::Search)` for `a.selected` and `a.search_query`, so the state
   behind them can be reshaped and split by feature without the tests following it.

The order matters within a run: model_paths.py moves the `Notice`, `Command` and `Event` that
the first two write, so a name main adds is converted in one run rather than two.

Each script rewrites only names that are still old, so running all five again changes nothing.
After a run, `cargo fmt --all` lays out the imports they rewrote, and the compiler names what is
left to do by hand.

Files main adds do not always follow the moves. One added in `crates/anistream-ui/` lands in
`crates/anistream-tui/`, and git asks to confirm it as a conflict. One added beside the old
library's modules in `crates/anistream/src/` stays there, because the binary's files kept that
directory; the engine's `lib.rs` then declares a module it cannot find, and the file is moved to
`crates/anistream-engine/src/` by hand.

The scripts are deleted after the final merge of main, when there is nothing left for them to
rewrite.
