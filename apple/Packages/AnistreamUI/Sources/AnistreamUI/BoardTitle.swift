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
