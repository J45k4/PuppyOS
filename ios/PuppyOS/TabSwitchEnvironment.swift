import SwiftUI

private struct TabSwitcherKey: EnvironmentKey {
    static let defaultValue: (AppTab) -> Void = { _ in }
}

extension EnvironmentValues {
    var tabSwitcher: (AppTab) -> Void {
        get { self[TabSwitcherKey.self] }
        set { self[TabSwitcherKey.self] = newValue }
    }
}

