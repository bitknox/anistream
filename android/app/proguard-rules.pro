# R8 for the release build. Everything the engine's bindings reach by name stays as it is.

# JNA finds its own classes, and the structures and callbacks it maps, by name and reflection.
-keep class com.sun.jna.** { *; }
-keep class * extends com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-dontwarn java.awt.**
-dontwarn com.sun.jna.**

# The UniFFI bindings: JNA binds their native functions by the method names, and their callback
# interfaces and structures are called and read from the engine's side.
-keep class tv.anistream.engine.** { *; }

# The player's mpv shim (player/src/main/cpp) finds MpvNative and its Listener by name as it
# loads, binds MpvNative's natives by name, and calls Listener.onEvent from mpv's event thread:
# nothing in Kotlin calls onEvent, so R8 would take it away and the shim would not load.
-keep class tv.anistream.player.MpvNative { native <methods>; }
-keep interface tv.anistream.player.MpvNative$Listener { *; }
