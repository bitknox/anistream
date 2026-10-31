// Static accessibility checks over the app's XAML, read as XML: no WinUI and no engine needed.
//
// The theme check is a hard test: App.xaml's HighContrast dictionary has to cover every brush the
// app's own pages use, and it may not hold a brand colour. The other two find known gaps that are
// being fixed page by page, so they only report (to the test output and, on CI, to the job
// summary). Set ANISTREAM_A11Y_STRICT=1 to make them fail. XAML is what the checks see: pages
// built in code are not covered.

using System.Text.RegularExpressions;
using System.Xml.Linq;
using Xunit;

namespace Anistream.App.Tests;

public class XamlA11yTests
{
    static readonly XNamespace X = "http://schemas.microsoft.com/winfx/2006/xaml";
    static readonly Regex Hex = new("^#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?$");

    // This file's own place in the checkout, as the compiler saw it, is the way to the app.
    static string AppDir([System.Runtime.CompilerServices.CallerFilePath] string here = "") =>
        Path.GetFullPath(Path.Combine(Path.GetDirectoryName(here)!, "..", "Anistream.Windows"));

    static IEnumerable<string> Pages() =>
        Directory.EnumerateFiles(AppDir(), "*.xaml", SearchOption.AllDirectories)
            .Where(p => !p.Contains(Path.DirectorySeparatorChar + "obj" + Path.DirectorySeparatorChar)
                     && !p.Contains(Path.DirectorySeparatorChar + "bin" + Path.DirectorySeparatorChar)
                     && Path.GetFileName(p) != "App.xaml");

    static XElement Theme(string key) =>
        XDocument.Load(Path.Combine(AppDir(), "App.xaml")).Descendants()
            .FirstOrDefault(e => e.Name.LocalName == "ResourceDictionary" && (string?)e.Attribute(X + "Key") == key)
        ?? throw new Xunit.Sdk.XunitException($"App.xaml has no \"{key}\" theme dictionary");

    static string Where(string file, XElement e)
    {
        var line = ((System.Xml.IXmlLineInfo)e).HasLineInfo() ? ((System.Xml.IXmlLineInfo)e).LineNumber : 0;
        return $"{Path.GetFileName(file)}:{line} <{e.Name.LocalName}>";
    }

    /// <summary>Strict fails; otherwise the findings go to the output and the job summary.</summary>
    static void Report(string check, List<string> findings)
    {
        if (findings.Count == 0) return;
        var text = $"{check}: {findings.Count} finding(s)\n" + string.Join("\n", findings.Select(f => "  " + f));
        TestContext.Current.TestOutputHelper?.WriteLine(text);
        Console.WriteLine("WARN " + text);
        var summary = Environment.GetEnvironmentVariable("GITHUB_STEP_SUMMARY");
        if (!string.IsNullOrEmpty(summary))
            File.AppendAllText(summary, $"### Windows XAML accessibility (warn only): {check}\n\n```\n{string.Join("\n", findings)}\n```\n");
        if (Environment.GetEnvironmentVariable("ANISTREAM_A11Y_STRICT") == "1") Assert.Fail(text);
    }

    [Fact]
    public void Every_app_brush_of_the_dark_theme_is_in_high_contrast()
    {
        // The app's own brushes follow the comment "The app's own." in the Dark dictionary.
        var dark = Theme("Dark");
        var own = dark.Nodes().SkipWhile(n => !(n is XComment c && c.Value.Contains("The app's own")))
            .OfType<XElement>().Select(e => (string?)e.Attribute(X + "Key")).Where(k => k != null).ToList();
        Assert.NotEmpty(own);
        var high = Theme("HighContrast").Elements().Select(e => (string?)e.Attribute(X + "Key")).ToHashSet();
        var missing = own.Where(k => !high.Contains(k)).ToList();
        Assert.True(missing.Count == 0, "HighContrast lacks: " + string.Join(", ", missing));
    }

    [Fact]
    public void High_contrast_holds_no_literal_colour()
    {
        // Pure black with an alpha is the one literal that is not a brand colour: a scrim's
        // darkness, where the system colours cannot say how translucent.
        var literal = new List<string>();
        foreach (var e in Theme("HighContrast").DescendantsAndSelf())
            foreach (var a in e.Attributes())
                if (Hex.IsMatch(a.Value) && !(a.Value.Length == 9 && a.Value.EndsWith("000000")))
                    literal.Add($"{e.Name.LocalName} {(string?)e.Attribute(X + "Key") ?? ""} {a.Name.LocalName}={a.Value}");
        Assert.True(literal.Count == 0, "HighContrast holds literal colours: " + string.Join("; ", literal));
    }

    [Fact]
    public void An_icon_only_button_has_an_automation_name()
    {
        static bool IsButton(XElement e) => e.Name.LocalName is "Button" or "AppBarButton" or "ToggleButton" or "AppBarToggleButton";
        var findings = new List<string>();
        foreach (var file in Pages())
            foreach (var e in XDocument.Load(file, LoadOptions.SetLineInfo).Descendants().Where(IsButton))
            {
                // Property elements (Button.Flyout, Button.Resources) are not content.
                var content = e.Elements().Where(c => !c.Name.LocalName.Contains('.')).ToList();
                foreach (var prop in e.Elements().Where(c => c.Name.LocalName == e.Name.LocalName + ".Content"))
                    content.AddRange(prop.Elements());
                var iconOnly = content.Count == 1 && content[0].Name.LocalName == "FontIcon"
                    && e.Attribute("Content") == null;
                var named = e.Attributes().Any(a => a.Name.LocalName == "AutomationProperties.Name" || a.Name.LocalName == "Label");
                if (iconOnly && !named) findings.Add(Where(file, e) + " has only a FontIcon and no AutomationProperties.Name");
            }
        Report("icon-only buttons", findings);
    }

    [Fact]
    public void Art_and_progress_inside_a_named_row_are_raw()
    {
        var findings = new List<string>();
        foreach (var file in Pages())
            foreach (var part in XDocument.Load(file, LoadOptions.SetLineInfo).Descendants().Where(d => d.Name.LocalName is "Image" or "ProgressBar"))
            {
                var row = part.Ancestors().FirstOrDefault(e => e.Attributes().Any(a => a.Name.LocalName == "AutomationProperties.Name"));
                var view = part.Attributes().FirstOrDefault(a => a.Name.LocalName == "AutomationProperties.AccessibilityView")?.Value;
                if (row != null && view != "Raw")
                    findings.Add(Where(file, part) + " is inside the named " + Where(file, row).Split(' ')[1] + " and is not AccessibilityView=\"Raw\"");
            }
        Report("art and progress in named rows", findings);
    }
}
