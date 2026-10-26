// A screen's title on iPad as the Pad and Mac boards draw it: small, at the top's leading edge.

#if os(iOS)
import AnistreamDesign
import SwiftUI

extension View {
    /// The screen's title as its board has it: on iPad the boards' small one, 17 semibold at the
    /// top's leading edge, in portrait and landscape alike; on iPhone and in a window narrow
    /// enough for the iPhone's screens, the large title the iPhone's boards draw.
    ///
    /// Not the bar's own inline title, which sits in the middle in landscape and gives way to the
    /// tab bar in portrait: the title is an item at the bar's leading edge, with no glass of its
    /// own, and the bar's title is taken out. The screen keeps `navigationTitle` for everything
    /// else that reads it.
    func boardTitle(_ title: String) -> some View {
        modifier(BoardTitle(title: title))
    }

    /// The line under a screen's large title, such as Continue's sync state, which can come and
    /// go while the screen is up.
    ///
    /// UIKit keeps the large title's height as it was when the line comes or goes, so a line that
    /// arrives once the screen is up sits squeezed under the title, and the screen's list starts
    /// where the shorter bar ended. The next pop back to the screen, a title's page zooming home
    /// into its cover among them, lays the bar out again at its full height: the list is then
    /// short of its top by the line, and UIKit takes that for a scroll and shows the title
    /// inline. Laying the bar out again as the line comes or goes keeps the two in step.
    func screenSubtitle(_ subtitle: String) -> some View {
        navigationSubtitle(subtitle)
            .background(BarLayout(subtitled: !subtitle.isEmpty))
    }
}

/// Lays out the navigation bar of the stack it is in whenever `subtitled` changes.
private struct BarLayout: UIViewRepresentable {
    let subtitled: Bool

    final class Coordinator {
        var subtitled: Bool?
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.isUserInteractionEnabled = false
        return view
    }

    func updateUIView(_ view: UIView, context: Context) {
        let was = context.coordinator.subtitled
        context.coordinator.subtitled = subtitled
        guard let was, was != subtitled else { return }
        // Once SwiftUI has handed the bar the line.
        DispatchQueue.main.async { [weak view] in
            var responder: UIResponder? = view
            while let next = responder, !(next is UIViewController) { responder = next.next }
            guard let stack = (responder as? UIViewController)?.navigationController else { return }
            stack.navigationBar.setNeedsLayout()
            stack.view.setNeedsLayout()
            stack.view.layoutIfNeeded()
        }
    }
}

struct BoardTitle: ViewModifier {
    @Environment(\.horizontalSizeClass) private var sizeClass
    let title: String

    func body(content: Content) -> some View {
        let small = sizeClass == .regular
        content
            .navigationBarTitleDisplayMode(small ? .inline : .automatic)
            .toolbar(removing: small ? .title : nil)
            .toolbar {
                if small {
                    ToolbarItem(placement: .topBarLeading) {
                        Text(title)
                            .font(.ui(17, weight: .semibold))
                            .foregroundStyle(.silk)
                            .fixedSize()
                            // 20 points in, where the large title started: the bar starts its
                            // first item at 10.
                            .padding(.leading, 10)
                            .accessibilityAddTraits(.isHeader)
                    }
                    .sharedBackgroundVisibility(.hidden)
                }
            }
    }
}
#endif
