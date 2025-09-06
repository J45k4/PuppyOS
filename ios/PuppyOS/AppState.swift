import Foundation
import SwiftUI

enum AppTab: Hashable {
    case track
    case timers
    case files
    case photos
}

final class AppState: ObservableObject {
    @Published var selectedTab: AppTab = .track
}
