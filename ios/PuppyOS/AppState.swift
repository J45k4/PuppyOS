import Foundation
import SwiftUI

enum AppTab: Hashable {
    case track
    case timers
}

final class AppState: ObservableObject {
    @Published var selectedTab: AppTab = .track
}

