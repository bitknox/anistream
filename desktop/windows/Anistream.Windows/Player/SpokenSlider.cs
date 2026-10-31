using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Player;

/// <summary>
/// A slider whose value a screen reader says in words: "12 minutes 4 seconds of 23 minutes
/// 40 seconds" for the seek bar, "50 percent" for the volume. The words come from <see cref="Spoken"/>
/// at the moment they are asked for, so they are right even when the thumb lags the playhead.
/// </summary>
public sealed class SpokenSlider : Slider
{
    /// <summary>The value as a person would say it.</summary>
    public Func<string>? Spoken { get; set; }

    protected override AutomationPeer OnCreateAutomationPeer() => new SpokenSliderPeer(this);

    sealed class SpokenSliderPeer(SpokenSlider owner) : SliderAutomationPeer(owner), IValueProvider
    {
        public string Value => owner.Spoken?.Invoke() ?? owner.Value.ToString("0");

        // Moving the slider goes through the range pattern, which the base peer keeps.
        public bool IsReadOnly => true;

        public void SetValue(string value)
        {
        }

        protected override object GetPatternCore(PatternInterface pattern) =>
            pattern == PatternInterface.Value ? this : base.GetPatternCore(pattern);
    }
}
