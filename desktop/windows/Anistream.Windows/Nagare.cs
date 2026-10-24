using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.UI;

namespace Anistream.Windows;

/// <summary>
/// Nagare 流, the mascot: the hand-authored pixel sprite, 20 by 32, the one the website and the
/// other apps draw. Shown in empty states and on first launch, never in navigation or the player.
/// Drawn as runs of rectangles, each sprite pixel <see cref="Cell"/> effective pixels, so he stays
/// crisp at every scale.
/// </summary>
sealed partial class Nagare : Canvas
{
    const int Width_ = 20, Height_ = 32;

    static readonly string[] Rows =
    [
        "........hH..........",
        ".........Hh.........",
        "........hH..........",
        "......hHHHHh........",
        ".....hHHHHHHh.......",
        "....hHHHHHHHHh......",
        "....HHHHHHHHHHh.....",
        "...hHFFFFhHHHHh.....",
        "...HFFFFFFhHHHh.....",
        "...HFEFFFFhHHHh.....",
        "...hFFFFFfhHHh......",
        ".ww.fFFFfhHHh.......",
        ".wWWAAAAAAHHh.......",
        ".wWaAAAAAAa..aAA....",
        "..w.aAAAa....aAAa...",
        "..SRCCCCCCc.........",
        "..SRCCCCCCCc........",
        "..SRCCCCCCCc........",
        "..SfFCCCCCCCc.......",
        "..SRCCCCCCCCc.......",
        "..SRCCCCCCCCc.......",
        "..SRCCCCCCCCcc......",
        "..SRCCCCCCCCCc......",
        "..SRCCCCCCCCCc......",
        "..SRCCCCCCCCCcc.....",
        "..SRCCCCCCCCCCc.....",
        "..SRCCCCCCCCCCc.....",
        "..SRCCCCCCCCCCcc....",
        "..SRCCCCCCCCCCCc....",
        "..S.BB......BB......",
        "....BB......BB......",
        "...BBB.....BBB......",
    ];

    static Color? Of(char c) => c switch
    {
        'H' => Rgb(0xE8, 0xE2, 0xD4),
        'h' => Rgb(0xC6, 0xBE, 0xA9),
        'F' => Rgb(0xEF, 0xCF, 0xAC),
        'f' => Rgb(0xD0, 0xA8, 0x83),
        'E' or 'B' => Rgb(0x23, 0x28, 0x42),
        'C' => Rgb(0x46, 0x4E, 0x7C),
        'c' => Rgb(0x2E, 0x34, 0x5C),
        'R' => Rgb(0x68, 0x72, 0x9E),
        'A' => Rgb(0xF2, 0xA6, 0x4B),
        'a' => Rgb(0xC5, 0x7C, 0x2C),
        'S' => Rgb(0x8A, 0x68, 0x44),
        'W' => Rgb(0x7F, 0xC7, 0xD9),
        'w' => Rgb(0xDF, 0xF5, 0xFB),
        _ => null,
    };

    static Color Rgb(byte r, byte g, byte b) => Color.FromArgb(0xFF, r, g, b);

    public Nagare() : this(4) { }

    public Nagare(double cell)
    {
        Cell = cell;
        Width = Width_ * cell;
        Height = Height_ * cell;
        AutomationProperties.SetName(this, "Nagare, the anistream mascot");
        var brushes = new Dictionary<char, SolidColorBrush>();
        for (var y = 0; y < Rows.Length; y++)
        {
            var row = Rows[y];
            for (var x = 0; x < row.Length;)
            {
                var c = row[x];
                var run = 1;
                while (x + run < row.Length && row[x + run] == c)
                    run++;
                if (Of(c) is { } colour)
                {
                    if (!brushes.TryGetValue(c, out var brush))
                        brushes[c] = brush = new SolidColorBrush(colour);
                    var rect = new Rectangle { Width = run * cell, Height = cell, Fill = brush };
                    SetLeft(rect, x * cell);
                    SetTop(rect, y * cell);
                    Children.Add(rect);
                }
                x += run;
            }
        }
    }

    public double Cell { get; }
}
