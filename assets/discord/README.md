# Discord art assets

Upload these in the Discord Developer Portal, under the anistream application:

- **General Information › App Icon:** `logo.png`.
- **Rich Presence › Art Assets:** all four, with each file's name (without `.png`) as the asset key: `logo`, `playing`, `paused` and `buffering`.

The presence sends `logo` as the large image when there is no cover. The other three are the small images that rich presence uses for its states.

`logo.png` is the app icon (`apple/App/Assets.xcassets/AppIcon.appiconset/ios-1024pt@1x.png`). The state icons are 512 px: amber `#F2A64B` on the ground colour `#161A2E`.
