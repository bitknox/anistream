// Light dismiss (native-ux.md): a surface with nothing to lose closes on a click outside it, and
// one that holds what the viewer typed stays open until a button closes it.
//
// macOS sheets never close on a click outside, so on the Mac an informational surface is a card in
// the window over the dimmed window instead, which a click on the dimming, Esc or its own Done
// closes. On iPhone and iPad it stays a sheet: a swipe down or a tap outside a form sheet closes
// it, until the viewer has typed or changed something.

import AnistreamDesign
import AnistreamStrings
import SwiftUI

/// The card's close, where a surface is a card rather than a sheet: `dismiss` has nothing to
/// dismiss there.
struct CloseSurface {
    let close: @MainActor () -> Void
}

extension EnvironmentValues {
    @Entry var closeSurface: CloseSurface?
}

/// Closes whatever holds the view: the Mac's informational card, or the sheet. Stands in for
/// `@Environment(\.dismiss)` in a view that can be either.
@MainActor
struct SurfaceDismiss: DynamicProperty {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.closeSurface) private var closeSurface

    func callAsFunction() {
        if let closeSurface { closeSurface.close() } else { dismiss() }
    }
}

/// A presence as a surface's item.
private struct Shown: Identifiable {
    var id: Int { 0 }
}

extension View {
    /// An informational surface: a card over the dimmed window on the Mac, a sheet elsewhere.
    func infoSurface<Item: Identifiable, Card: View>(
        item: Binding<Item?>,
        @ViewBuilder content: @escaping (Item) -> Card
    ) -> some View {
        #if os(macOS)
        modifier(InfoCard(item: item, card: content))
        #else
        sheet(item: item, content: content)
        #endif
    }

    /// An informational surface shown while `isPresented` is on.
    func infoSurface<Card: View>(
        isPresented: Binding<Bool>,
        @ViewBuilder content: @escaping () -> Card
    ) -> some View {
        infoSurface(item: Binding<Shown?>(
            get: { isPresented.wrappedValue ? Shown() : nil },
            set: { if $0 == nil { isPresented.wrappedValue = false } }
        )) { _ in content() }
    }

    /// A sheet holding input: a swipe down or a tap outside still closes it on iPhone and iPad
    /// until `value` changes, and from then on only its buttons do. The Mac's sheets never close on
    /// a click outside.
    func holdsInput<Value: Equatable>(_ value: Value) -> some View {
        modifier(HoldsInput(value: value))
    }

    /// A sheet holding input, held open while `typed` is on.
    func holdsInput(when typed: Bool) -> some View {
        #if os(iOS)
        interactiveDismissDisabled(typed)
        #else
        self
        #endif
    }
}

private struct HoldsInput<Value: Equatable>: ViewModifier {
    let value: Value
    @State private var changed = false

    func body(content: Content) -> some View {
        #if os(iOS)
        content
            .interactiveDismissDisabled(changed)
            .onChange(of: value) { changed = true }
        #else
        content
        #endif
    }
}

#if os(macOS)
/// The Mac's informational card: centred over the window, which dims behind it. A click on the
/// dimming, Esc or the card's Done closes it. It fades in and out (fade-in, fade-out), and stays
/// inside the window with a margin.
private struct InfoCard<Item: Identifiable, Card: View>: ViewModifier {
    @Binding var item: Item?
    let card: (Item) -> Card
    @FocusState private var focused: Bool

    func body(content: Content) -> some View {
        content.overlay {
            GeometryReader { proxy in
                ZStack {
                    if let shown = item {
                        Palette.ground.opacity(0.35)
                            .ignoresSafeArea()
                            .contentShape(.rect)
                            .onTapGesture { close() }
                            .accessibilityHidden(true)
                            .transition(.opacity)
                        card(shown)
                            .environment(\.closeSurface, CloseSurface { close() })
                            .clipShape(.rect(cornerRadius: 12))
                            .overlay { RoundedRectangle(cornerRadius: 12).strokeBorder(Palette.glassRule) }
                            // The card's own size, offered no more than the window less a margin.
                            .frame(maxWidth: max(0, proxy.size.width - 64), maxHeight: max(0, proxy.size.height - 64))
                            .focusable()
                            .focusEffectDisabled()
                            .focused($focused)
                            .onAppear { focused = true }
                            .onExitCommand { close() }
                            .accessibilityAddTraits(.isModal)
                            .accessibilityAction(.escape) { close() }
                            .background {
                                // Esc wherever the focus is, as a sheet's Cancel would.
                                Button(L.actionClose) { close() }
                                    .keyboardShortcut(.cancelAction)
                                    .opacity(0)
                                    .accessibilityHidden(true)
                            }
                            .transition(.opacity)
                            .id(shown.id)
                    }
                }
                .frame(width: proxy.size.width, height: proxy.size.height)
            }
            .animation(item == nil ? .fadeOut : .fadeIn, value: item?.id)
        }
    }

    private func close() { item = nil }
}
#endif
