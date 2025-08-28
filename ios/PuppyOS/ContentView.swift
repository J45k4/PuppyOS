//
//  ContentView.swift
//  PuppyOS
//
//  Created by puppy on 28.8.2025.
//

import SwiftUI

struct TimeEntry: Identifiable, Codable {
    let id: UUID
    var title: String
    var start: Date
    var end: Date
    
    var duration: TimeInterval { end.timeIntervalSince(start) }
}

struct ContentView: View {
    @State private var isPlaying = false
    @State private var description: String = ""
    @State private var entries: [TimeEntry] = []
    @State private var currentStart: Date? = nil
    @State private var now: Date = Date()
    @State private var showingNewEntry: Bool = false
    let ticker = Timer.publish(every: 1, on: .main, in: .common).autoconnect()
    
    private let entriesKey = "TimeEntries"
    
    var body: some View {
        NavigationStack {
        VStack(spacing: 12) {
            Button(action: togglePlay) {
                Image(systemName: isPlaying ? "stop.fill" : "play.fill")
                    .font(.system(size: 50))
                    .foregroundColor(.blue)
                    .accessibilityLabel(isPlaying ? "Stop tracking" : "Start tracking")
            }
            .padding(.top)

            // Multiline description input
            ZStack(alignment: .topLeading) {
                if description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    Text("What are you doing?")
                        .foregroundColor(.secondary)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 12)
                        .allowsHitTesting(false)
                }
                TextEditor(text: $description)
                    .font(.body)
                    .frame(height: 72)
                    .padding(6)
                    .accessibilityLabel("Task description")
            }
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(Color.secondary.opacity(0.3), lineWidth: 1)
            )
            .padding(.horizontal)

            // Current running timer display
            if isPlaying, let start = currentStart {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Image(systemName: "clock.fill")
                            .foregroundColor(.green)
                        Text("\(description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "(No title)" : description) · \(formattedDuration(now.timeIntervalSince(start)))")
                            .font(.subheadline)
                            .monospacedDigit()
                            .foregroundColor(.primary)
                    }
                    // Allow adjusting start time while running
                    DatePicker(
                        "Start",
                        selection: Binding(
                            get: { currentStart ?? Date() },
                            set: { newVal in
                                // Prevent setting start in the future
                                currentStart = min(newVal, now)
                            }
                        ),
                        in: ...now,
                        displayedComponents: [.date, .hourAndMinute]
                    )
                    .datePickerStyle(.compact)
                }
                .padding(.horizontal)
            }

            // List of saved time entries under the text input
            List {
                ForEach(entries.sorted(by: { $0.start > $1.start })) { entry in
                    NavigationLink {
                        EntryDetailView(entry: entry) { updated in
                            updateEntry(updated)
                        }
                    } label: {
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(entry.title.isEmpty ? "(No title)" : entry.title)
                                    .font(.headline)
                                Text("\(formattedDate(entry.start)) · \(formattedDuration(entry.duration))")
                                    .font(.subheadline)
                                    .foregroundColor(.secondary)
                            }
                            Spacer()
                        }
                        .padding(.vertical, 2)
                    }
                }
            }
            .listStyle(.plain)
        }
        .navigationTitle("Time Tracker")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    showingNewEntry = true
                } label: {
                    Image(systemName: "plus")
                }
                .accessibilityLabel("Add time entry")
            }
        }
        .onAppear(perform: loadEntries)
        .onReceive(ticker) { value in
            now = value
        }
        .padding()
        }
        .sheet(isPresented: $showingNewEntry) {
            NavigationStack {
                EntryDetailView(
                    entry: TimeEntry(id: UUID(), title: "", start: Date(), end: Date())
                ) { newEntry in
                    entries.append(newEntry)
                    saveEntries()
                }
            }
        }
    }
    
    private func togglePlay() {
        if isPlaying {
            // Stop and save entry
            let start = currentStart ?? Date()
            let end = Date()
            let entry = TimeEntry(id: UUID(), title: description.trimmingCharacters(in: .whitespacesAndNewlines), start: start, end: end)
            entries.append(entry)
            saveEntries()
            currentStart = nil
            isPlaying = false
        } else {
            // Start tracking
            currentStart = Date()
            isPlaying = true
        }
    }
    
    private func saveEntries() {
        do {
            let data = try JSONEncoder().encode(entries)
            UserDefaults.standard.set(data, forKey: entriesKey)
        } catch {
            print("Failed to save entries: \(error)")
        }
    }
    
    private func updateEntry(_ updated: TimeEntry) {
        if let idx = entries.firstIndex(where: { $0.id == updated.id }) {
            entries[idx] = updated
            saveEntries()
        }
    }
    
    private func loadEntries() {
        guard let data = UserDefaults.standard.data(forKey: entriesKey) else { return }
        do {
            let decoded = try JSONDecoder().decode([TimeEntry].self, from: data)
            entries = decoded
        } catch {
            print("Failed to load entries: \(error)")
        }
    }
    
    private func formattedDate(_ date: Date) -> String {
        let f = DateFormatter()
        f.dateStyle = .short
        f.timeStyle = .short
        return f.string(from: date)
    }
    
    private func formattedDuration(_ interval: TimeInterval) -> String {
        let f = DateComponentsFormatter()
        f.allowedUnits = [.hour, .minute, .second]
        f.unitsStyle = .abbreviated
        f.zeroFormattingBehavior = .pad
        return f.string(from: interval) ?? "0s"
    }
}

#Preview {
    ContentView()
}
