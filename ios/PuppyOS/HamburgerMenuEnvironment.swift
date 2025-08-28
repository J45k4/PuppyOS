import SwiftUI

private struct ToggleMenuKey: EnvironmentKey {
    static let defaultValue: () -> Void = {}
}

extension EnvironmentValues {
    var toggleMenu: () -> Void {
        get { self[ToggleMenuKey.self] }
        set { self[ToggleMenuKey.self] = newValue }
    }
}

