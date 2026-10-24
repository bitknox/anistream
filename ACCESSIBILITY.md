# Accessibility

This page says what accessibility support is built into each part of anistream, what is missing,
and how to report a barrier. It describes what the code does and what is tested. anistream has
not been audited against WCAG or any other standard, and this page makes no claim of conformance.

## Mac, iPhone and iPad

The three apps share one SwiftUI codebase, so they share this support.

- VoiceOver and Voice Control: covers and list rows are one element named for the title, with
  progress read in words ("Episode 7 of 24", "14 minutes left"), and their context menu items
  are available as actions. Romaji titles are read in Japanese. Notices are announced.
- Rotors for unwatched episodes, days of the calendar and items in progress, so VoiceOver users
  can jump between them.
- Dynamic Type: text follows the system size. Some text is capped at a largest size so layouts
  hold.
- Reduce Motion and Reduce Transparency are honored. With Reduce Transparency on, the panels over
  video and artwork become solid.
- Keyboard: the Mac app has shortcuts and arrow-key navigation in lists and the command palette.
- Captions: subtitles in plain-text formats follow the system caption style (font, size,
  colours, background and edge) while that setting is on in Settings. Styled (ASS) subtitles keep
  their own look.
- Testing: `apple/AuditTests` runs Xcode's accessibility audit on the main screens, including at
  the largest accessibility text size. It is an on-demand test suite. Findings that come from a
  deliberate design choice are set aside in the test with a reason.

## Android

- TalkBack: controls and covers carry content descriptions and state descriptions, headings are
  marked, and the player's controls and the Up Next card are designed to wait for you instead of
  counting down while TalkBack or Switch Access is on (see `android/docs/motion-android.md`).
- Font scale: the layouts change at large font sizes. The screenshot tests record the main
  screens at 2.0 times font size.
- Animations: the app never overrides the system's animation scale. With "Remove animations" on,
  timed motion lands at once; `android/docs/motion-android.md` states this for each screen.
- Captions: subtitles in plain-text formats follow the system caption style while captions are
  on in Android's accessibility settings.

## Android TV

Coming. The TV app is on the `tv/app` branch and is not released. It is built for the remote's
directional pad, with focus handling on each screen. This section will be filled in when it ships.

## Terminal app

- It is driven from the keyboard, with a key map you can change under `[keys]` in
  `config.toml`.
- It does not draw anything that needs a patched font.

Gaps: there is no screen reader mode, it does not read `NO_COLOR`, and nobody has checked
screen by screen which state it shows by colour alone.

## Website

- Keyboard focus is outlined everywhere with `:focus-visible`.
- With `prefers-reduced-motion`, the site's transitions and fades are set to zero length.
- Screenshots have alt text that describes them.

## Plugin store admin page

- The page is in English and marked with its language. Form fields have labels, sections are
  named by their headings, and decorative marks are hidden from assistive technology.
- Buttons and links show a focus outline.
- The animated background is skipped with `prefers-reduced-motion`.

## Known gaps

- Android TV is not released yet.
- The terminal app has no screen reader mode and ignores `NO_COLOR`.
- The Apple audit test is not part of the checks CI runs, and there is no equivalent automated
  audit for Android, the website or the admin page.
- No one has tested the website or the admin page with a screen reader as part of the project's
  checks.
- Styled (ASS) subtitles ignore the system caption style.

## Report a barrier

[Open an accessibility issue](https://github.com/bitknox/anistream/issues/new?template=accessibility.yml).
Say which app and version you use, your platform and assistive technology, what you tried, and
what happened. Don't link to sources or name sites that carry content. If you'd rather not use
GitHub's issue tracker, contact the maintainer [@bitknox](https://github.com/bitknox) privately
on GitHub.
