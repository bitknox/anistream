# Accessibility

This page says what accessibility support is built into each part of anistream, what is missing,
and how to report a barrier. It describes what the code does and what is tested. anistream has
not been audited against WCAG or any other standard, and this page makes no claim of conformance.

## Mac, iPhone and iPad

The three apps share one SwiftUI codebase, so they share this support.

| Area | What the apps do |
| --- | --- |
| VoiceOver and Voice Control | Covers and list rows are one element named for the title, with progress read in words ("Episode 7 of 24", "14 minutes left"), and their context menu items are available as actions. Romaji titles are read in Japanese. Notices are announced. |
| Rotors | Rotors for unwatched episodes, days of the calendar and items in progress let VoiceOver users jump between them. |
| Dynamic Type | Text follows the system size. Some text is capped at a largest size so layouts hold. |
| Reduce Motion and Reduce Transparency | Both are honoured. With Reduce Transparency on, the panels over video and artwork become solid. |
| Keyboard | The Mac app has shortcuts and arrow-key navigation in lists and the command palette. |
| Captions | Subtitles in plain-text formats follow the system caption style (font, size, colours, background and edge) while that setting is on in Settings. Styled (ASS) subtitles keep their own look. |
| Testing | `apple/AuditTests` runs Xcode's accessibility audit on the main screens, including at the largest accessibility text size. It is an on-demand test suite. Findings that come from a design choice are set aside in the test with a reason. |

## Android

- TalkBack: controls and covers carry content descriptions and state descriptions, headings are
  marked, and the player's controls and the Up next card are designed to wait for you instead of
  counting down while TalkBack or Switch Access is on (see `android/docs/motion-android.md`).
- Font scale: the layouts change at large font sizes. The screenshot tests record the main
  screens at 2.0 times font size.
- Animations: the app never overrides the system's animation scale. With "Remove animations" on,
  timed motion lands at once; `android/docs/motion-android.md` states this for each screen.
- Captions: subtitles in plain-text formats follow the system caption style while captions are
  on in Android's accessibility settings.

## Android TV

The Android TV app ships in every release. It is built for the remote's directional pad, and
every screen is driven by it: the rail, the browse rows, the title page, Settings, sign-in and the
player. The focused item has a visible ring, and the app asks the focus where it needs to go
when a screen opens.

- TalkBack reads a card as one element, with its title, its line and its progress in words. The
  actions in a card's panel are in TalkBack's actions menu. Icon buttons carry a spoken label.
- Page and section titles are marked as headings.
- Notices are live regions, so TalkBack says them as they appear.
- The app reads the system's animation scale. With animations off, fades and the focus scale
  land at once.

Gaps: the player's seek bar has no label for TalkBack, and some panels are a fixed width, so large
system text can crowd them. The TV has no high contrast mode.

## Terminal app

- It is driven from the keyboard, with a key map you can change under `[keys]` in
  `config.toml`.
- It does not draw anything that needs a patched font.

Gaps: there is no screen reader mode, it does not read `NO_COLOR`, and nobody has checked
screen by screen which state it shows by colour alone.

## Windows

The Windows app is a WinUI 3 app. It sets screen-reader names on its custom controls, says
status changes to Narrator as polite or assertive announcements, and works from the keyboard.
Text follows the system text size. With "Animation effects" off in Windows, motion lands at once.

Gaps: it does not change its look in Windows high contrast themes, and subtitles do not follow a
system caption style. Nobody has reviewed it with Narrator screen by screen.

## Linux

The Linux app is a GTK 4 app. It sets accessible labels on its custom controls, announces
changes such as a sign-in link arriving, and works from the keyboard. Text follows the system
text size. It turns off animations when GTK's animations setting is off, and it uses a second
style sheet while GTK is in high contrast.

Gaps: subtitles do not follow a system caption style. Nobody has reviewed it with Orca screen by
screen.

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

- The terminal app has no screen reader mode and ignores `NO_COLOR`.
- The Apple audit test is not part of the checks CI runs, and there is no equivalent automated
  audit for Android, the website or the admin page.
- No one has tested the website or the admin page with a screen reader as part of the project's
  checks.
- Styled (ASS) subtitles ignore the system caption style. On Windows and Linux, no subtitles
  follow it.
- The Android phone, tablet and TV apps and the Windows app have no high contrast mode.

## Report a barrier

[Open an accessibility issue](https://github.com/bitknox/anistream/issues/new?template=accessibility.yml).
Say which app and version you use, your platform and assistive technology, what you tried, and
what happened. Don't link to sources or name sites that carry content. If you'd rather not use
GitHub's issue tracker, contact the maintainer [@bitknox](https://github.com/bitknox) privately
on GitHub.
