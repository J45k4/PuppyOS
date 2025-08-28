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
    @StateObject private var appState = AppState()
    @State private var showMenu = false
    var body: some Scene {
        WindowGroup {
            ZStack(alignment: .leading) {
                TabView(selection: $appState.selectedTab) {
                    ContentView()
                        .tabItem { Label("Track", systemImage: "clock.arrow.circlepath") }
                        .tag(AppTab.track)
                    TimersView()
                        .tabItem { Label("Timers", systemImage: "timer") }
                        .tag(AppTab.timers)
                }
                .disabled(showMenu)

                if showMenu {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                        .onTapGesture { withAnimation(.easeInOut) { showMenu = false } }
                }
                SideMenuView { item in
                    switch item {
                    case .track: appState.selectedTab = .track
                    case .timers: appState.selectedTab = .timers
                    }
                    withAnimation(.easeInOut) { showMenu = false }
                }
                .offset(x: showMenu ? 0 : -320)
                .animation(.easeInOut(duration: 0.2), value: showMenu)
            }
            .environment(\.managedObjectContext, persistenceController.container.viewContext)
            .environment(\.tabSwitcher, { tab in appState.selectedTab = tab })
            .environment(\.toggleMenu, { withAnimation(.easeInOut) { showMenu.toggle() } })
            .onAppear { NotificationManager.requestAuthorization() }
        }
    }
}
