//
//  PuppyOSApp.swift
//  PuppyOS
//
//  Created by puppy on 28.8.2025.
//

import SwiftUI
import CoreData

@main
struct PuppyOSApp: App {
    let persistenceController = PersistenceController.shared
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(\.managedObjectContext, persistenceController.container.viewContext)
        }
    }
}
