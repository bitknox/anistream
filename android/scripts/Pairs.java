// Sets each screenshot beside the board it was built to, for android/scripts/pairs.sh: the board
// on the left, the app on the right, both at one height on the app's ground, each captioned.
//
//   java android/scripts/Pairs.java <boards> <shots> <out>
//
// <boards> holds the boards as PNGs by name (AndroidHome.png), <shots> the screenshot tests'
// captures (phone-home.png), and <out> gets pair-<board>.png for every board with a capture.

import java.awt.Color;
import java.awt.Font;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.IOException;
import java.util.LinkedHashMap;
import java.util.Map;
import javax.imageio.ImageIO;

public class Pairs {
    /** Each capture and its board, with the scene it was drawn from. */
    record Pair(String shot, String board, String scene, int height) {}

    static final Pair[] PAIRS = {
        new Pair("phone-welcome", "AndroidWelcome", "Fresh scene", 1500),
        new Pair("phone-home", "AndroidHome", "Watching scene", 1500),
        new Pair("phone-title", "AndroidTitle", "Watching scene, title 900001", 1500),
        new Pair("phone-title", "AndroidTitleMeta", "Watching scene, title 900001", 1500),
        new Pair("phone-more-about", "AndroidMoreAbout", "Watching scene, title 900001, More about this show", 1500),
        new Pair("phone-show-settings", "AndroidShowSettings", "Watching scene, title 900001, Settings for this show", 1500),
        new Pair("phone-calendar", "AndroidCalendar", "Watching scene, My list", 1500),
        new Pair("phone-search", "AndroidSearch", "Watching scene, query hoshi", 1500),
        new Pair("phone-library", "AndroidLibrary", "Watching scene, signed in to the fake AniList", 1500),
        new Pair("tablet-home", "AndroidTabletHome", "Watching scene", 800),
        new Pair("tablet-title", "AndroidTabletTitle", "Watching scene, title 900001", 800),
        new Pair("phone-settings", "AndroidSettings", "Watching scene, Settings", 1500),
        new Pair("phone-settings-playback", "AndroidSettingsPlayback", "Watching scene, Settings, Playback", 1500),
        new Pair("phone-settings-titles", "AndroidSettingsTitles", "Watching scene, Playback, title language menu", 1500),
        new Pair("phone-settings-sources", "AndroidSettingsSources", "Watching scene, Settings, Sources", 1500),
        new Pair("tablet-settings", "AndroidTabletSettings", "Watching scene, Settings, Playback", 800),
        new Pair("phone-providers", "AndroidProviders", "Watching scene, the boards' five sources", 1500),
        new Pair("phone-store", "AndroidStore", "Watching scene, the boards' store", 1500),
        new Pair("phone-join-store", "AndroidJoinStore", "Watching scene, an invite checked", 1500),
        new Pair("tablet-more-about", "AndroidTabletMoreAbout", "Watching scene, title 900001, More about this show", 800),
    };

    static final Color GROUND = new Color(0x0d0f1b);
    static final Color CAPTION = new Color(0x9298b5);
    static final int PAD = 24;
    static final int GAP = 32;
    static final int CAPTION_HEIGHT = 52;

    public static void main(String[] args) throws IOException {
        if (args.length != 3) {
            System.err.println("usage: java Pairs.java <boards> <shots> <out>");
            System.exit(2);
        }
        File boards = new File(args[0]), shots = new File(args[1]), out = new File(args[2]);
        out.mkdirs();
        Map<String, String> missing = new LinkedHashMap<>();
        for (Pair pair : PAIRS) {
            File board = new File(boards, pair.board + ".png");
            File shot = new File(shots, pair.shot + ".png");
            if (!board.isFile()) { missing.put(pair.board, board.getPath()); continue; }
            if (!shot.isFile()) { missing.put(pair.shot, shot.getPath()); continue; }
            File written = new File(out, "pair-" + pair.board + ".png");
            ImageIO.write(pair(ImageIO.read(board), "Board: " + pair.board, ImageIO.read(shot), "JVM: fixtures, " + pair.scene, pair.height), "png", written);
            System.out.println("pairs: wrote " + written.getPath());
        }
        missing.forEach((name, path) -> System.err.println("pairs: no " + name + " at " + path));
    }

    static BufferedImage pair(BufferedImage left, String leftCaption, BufferedImage right, String rightCaption, int height) {
        int leftWidth = left.getWidth() * height / left.getHeight();
        int rightWidth = right.getWidth() * height / right.getHeight();
        BufferedImage canvas = new BufferedImage(PAD + leftWidth + GAP + rightWidth + PAD, PAD + height + CAPTION_HEIGHT + PAD, BufferedImage.TYPE_INT_RGB);
        Graphics2D g = canvas.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_INTERPOLATION, RenderingHints.VALUE_INTERPOLATION_BICUBIC);
        g.setRenderingHint(RenderingHints.KEY_RENDERING, RenderingHints.VALUE_RENDER_QUALITY);
        g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
        g.setColor(GROUND);
        g.fillRect(0, 0, canvas.getWidth(), canvas.getHeight());
        g.drawImage(left, PAD, PAD, leftWidth, height, null);
        g.drawImage(right, PAD + leftWidth + GAP, PAD, rightWidth, height, null);
        g.setColor(CAPTION);
        g.setFont(new Font(Font.SANS_SERIF, Font.PLAIN, 22));
        int baseline = PAD + height + 34;
        g.drawString(leftCaption, PAD, baseline);
        g.drawString(rightCaption, PAD + leftWidth + GAP, baseline);
        g.dispose();
        return canvas;
    }
}
