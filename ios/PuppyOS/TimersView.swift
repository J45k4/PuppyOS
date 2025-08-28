import SwiftUI
import CoreData

struct TimersView: View {
    @Environment(\.managedObjectContext) private var context
    @Environment(\.tabSwitcher) private var switchTab
    @Environment(\.toggleMenu) private var toggleMenu
    @FetchRequest(
        entity: TimerEntity.entity(),
        sortDescriptors: [NSSortDescriptor(keyPath: \TimerEntity.end, ascending: true)],
        animation: .default
    ) private var timers: FetchedResults<TimerEntity>
    @State private var now: Date = Date()
    let ticker = Timer.publish(every: 1, on: .main, in: .common).autoconnect()
    @State private var showingNew: Bool = false
    @State private var showAlert: Bool = false
    @State private var alertText: String = ""
    
    var body: some View {
        NavigationStack {
            List {
                ForEach(timers) { timer in
                    HStack {
                        VStack(alignment: .leading) {
                            Text(timer.title.isEmpty ? "(Untitled)" : timer.title)
                                .font(.headline)
                            Text(remainingString(for: timer))
                                .font(.subheadline)
                                .monospacedDigit()
                                .foregroundColor(timer.isRunning ? .secondary : Color.secondary.opacity(0.4))
                        }
                        Spacer()
                        if timer.isRunning {
                            Button(role: .destructive) { stop(timer) } label: {
                                Image(systemName: "stop.fill")
                            }.buttonStyle(.bordered)
                        } else {
                            Button { start(timer) } label: {
                                Image(systemName: "play.fill")
                            }.buttonStyle(.borderedProminent)
                        }
                    }
                }
                .onDelete(perform: delete)
            }
            .navigationTitle("Timers")
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button { toggleMenu() } label: { Image(systemName: "line.3.horizontal") }
                        .accessibilityLabel("Open menu")
                }
                ToolbarItem(placement: .primaryAction) {
                    Button { showingNew = true } label: { Image(systemName: "plus") }
                }
            }
        }
        .onReceive(ticker) { newNow in
            now = newNow
            handleCompletedTimers()
        }
        .sheet(isPresented: $showingNew) { NewTimerView() }
        .onAppear { NotificationManager.requestAuthorization() }
        .alert(isPresented: $showAlert) {
            Alert(title: Text("Timer Finished"), message: Text(alertText), dismissButton: .default(Text("OK")))
        }
    }
    
    private func remainingString(for timer: TimerEntity) -> String {
        let remaining = max(0, timer.end.timeIntervalSince(now))
        let f = DateComponentsFormatter()
        f.allowedUnits = [.hour, .minute, .second]
        f.unitsStyle = .positional
        f.zeroFormattingBehavior = [.pad]
        return f.string(from: remaining) ?? "00:00"
    }
    
    private func start(_ timer: TimerEntity) {
        timer.isRunning = true
        if timer.end <= Date() {
            // If end is in the past, set to a minute from now
            timer.end = Date().addingTimeInterval(60)
        }
        let notifId = timer.notificationId ?? UUID().uuidString
        timer.notificationId = notifId
        NotificationManager.scheduleTimerNotification(id: notifId, title: timer.title, fireDate: timer.end)
        try? context.save()
    }
    
    private func stop(_ timer: TimerEntity) {
        timer.isRunning = false
        if let id = timer.notificationId { NotificationManager.cancelNotification(id: id) }
        timer.notificationId = nil
        try? context.save()
    }
    
    private func delete(_ offsets: IndexSet) {
        for i in offsets { 
            let t = timers[i]
            if let id = t.notificationId { NotificationManager.cancelNotification(id: id) }
            context.delete(t)
        }
        try? context.save()
    }
    
    private func handleCompletedTimers() {
        // Clear running flag for completed timers when app is in foreground
        let list = timers.filter { $0.isRunning && $0.end <= now }
        guard !list.isEmpty else { return }
        for t in list {
            t.isRunning = false
            if let id = t.notificationId { NotificationManager.cancelNotification(id: id) }
            t.notificationId = nil
        }
        try? context.save()
        // Present a simple alert for the first completed timer when app is foregrounded
        if let first = list.first {
            alertText = first.title.isEmpty ? "A timer has completed." : first.title
            showAlert = true
        }
    }
}

struct NewTimerView: View {
    @Environment(\.managedObjectContext) private var context
    @Environment(\.dismiss) private var dismiss
    @State private var title: String = ""
    @State private var hours: Int = 0
    @State private var minutes: Int = 5
    @State private var seconds: Int = 0
    
    var body: some View {
        NavigationStack {
            Form {
                Section("Title") {
                    TextField("Timer name (optional)", text: $title)
                }
                Section("Duration") {
                    HStack {
                        Stepper("Hours: \(hours)", value: $hours, in: 0...23)
                    }
                    HStack {
                        Stepper("Minutes: \(minutes)", value: $minutes, in: 0...59)
                    }
                    HStack {
                        Stepper("Seconds: \(seconds)", value: $seconds, in: 0...59)
                    }
                    HStack {
                        Text("Total")
                        Spacer()
                        Text(totalString)
                            .monospacedDigit()
                            .foregroundColor(.secondary)
                    }
                }
            }
            .navigationTitle("New Timer")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }
                        .disabled(totalSeconds == 0)
                }
            }
        }
    }
    
    private var totalSeconds: TimeInterval { TimeInterval(hours*3600 + minutes*60 + seconds) }
    private var totalString: String {
        let f = DateComponentsFormatter()
        f.allowedUnits = [.hour, .minute, .second]
        f.unitsStyle = .positional
        f.zeroFormattingBehavior = [.pad]
        return f.string(from: totalSeconds) ?? "00:00"
    }
    
    private func save() {
        let t = TimerEntity(context: context)
        t.id = UUID()
        t.title = title.trimmingCharacters(in: .whitespacesAndNewlines)
        t.end = Date().addingTimeInterval(totalSeconds)
        t.isRunning = true
        let notifId = UUID().uuidString
        t.notificationId = notifId
        NotificationManager.scheduleTimerNotification(id: notifId, title: t.title, fireDate: t.end)
        try? context.save()
        dismiss()
    }
}

#Preview {
    let pc = PersistenceController(inMemory: true)
    return TimersView()
        .environment(\.managedObjectContext, pc.container.viewContext)
        .environment(\.tabSwitcher, { _ in })
        .environment(\.toggleMenu, { })
}
