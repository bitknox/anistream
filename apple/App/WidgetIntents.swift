// The widgets' play button runs in the app: its intent is AnistreamWidgetData's, which the app
// takes in here, and the app's routes take the link it opens (AnistreamUI's WidgetRoutes).

import AnistreamWidgetData
import AppIntents

struct AnistreamAppIntentsPackage: AppIntentsPackage {
    static var includedPackages: [any AppIntentsPackage.Type] {
        [AnistreamWidgetIntents.self]
    }
}
