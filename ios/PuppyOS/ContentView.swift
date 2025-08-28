//
//  ContentView.swift
//  PuppyOS
//
//  Created by puppy on 28.8.2025.
//

import SwiftUI
import CoreData

struct ContentView: View {
    @Environment(\.managedObjectContext) private var context
    @State private var isPlaying = false
    @State private var description: String = ""
    @FetchRequest(
        sortDescriptors: [NSSortDescriptor(keyPath: \TimeEntryEntity.start, ascending: false)],
        animation: .default
    ) private var entries: FetchedResults<TimeEntryEntity>
    @State private var currentStart: Date? = nil
    @State private var now: Date = Date()
    @State private var showingNewEntry: Bool = false
    let ticker = Timer.publish(every: 1, on: .main, in: .common).autoconnect()
    // Keys for persisting the in-progress tracking session
    private let currentStartKey = "CurrentTrackingStart"
    private let currentDescKey = "CurrentTrackingDescription"
    
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

            // Suggestions from past entries matching current description
            if !matchingSuggestions.isEmpty {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(matchingSuggestions, id: \.self) { suggestion in
                        Button {
                            description = suggestion
                        } label: {
                            HStack {
                                Image(systemName: "text.magnifyingglass")
                                    .foregroundColor(.secondary)
                                Text(suggestion)
                                    .foregroundColor(.primary)
                                    .lineLimit(1)
                                    .truncationMode(.tail)
                                Spacer()
                            }
                            .padding(.horizontal, 10)
                            .padding(.vertical, 8)
                        }
                        .buttonStyle(.plain)
                        if suggestion != matchingSuggestions.last { Divider() }
                    }
                }
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .fill(.background)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(Color.secondary.opacity(0.3), lineWidth: 1)
                )
                .padding(.horizontal)
            }

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
                                persistCurrentTracking()
                            }
                        ),
                        in: ...now,
                        displayedComponents: [.date, .hourAndMinute]
                    )
                    .datePickerStyle(.compact)

                    HStack {
                        Spacer()
                        Button(role: .destructive) {
                            discardCurrent()
                        } label: {
                            Label("Discard", systemImage: "trash")
                        }
                        .buttonStyle(.bordered)
                        .tint(.red)
                    }
                }
                .padding(.horizontal)
            }

            // List of saved time entries under the text input
            List {
                ForEach(entries) { entry in
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        NavigationLink {
                            EntryDetailView(entry: entry) {
                                context.delete(entry)
                                try? context.save()
                            }
                        } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(entry.title.isEmpty ? "(No title)" : entry.title)
                                    .font(.headline)
                                Text("\(formattedDate(entry.start)) · \(formattedDuration(entry.duration))")
                                    .font(.subheadline)
                                    .foregroundColor(.secondary)
                            }
                        }
                        Spacer(minLength: 8)
                        Button(action: { startTracking(from: entry) }) {
                            Image(systemName: "play.fill")
                                .imageScale(.medium)
                        }
                        .buttonStyle(.bordered)
                        .tint(.blue)
                        .accessibilityLabel("Start new tracking with this entry")
                    }
                    .padding(.vertical, 2)
                }
                .onDelete(perform: deleteOffsets)
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
        .onReceive(ticker) { value in
            now = value
        }
        .padding()
        }
        .sheet(isPresented: $showingNewEntry) {
            NavigationStack {
                NewEntryView()
            }
        }
        .onAppear(perform: loadCurrentTracking)
        .onChange(of: description) { _ in
            if isPlaying { persistCurrentTracking() }
        }
    }
    
    private func togglePlay() {
        if isPlaying {
            // Stop and save entry
            let start = currentStart ?? Date()
            let end = Date()
            let obj = TimeEntryEntity(context: context)
            obj.id = UUID()
            obj.title = description.trimmingCharacters(in: .whitespacesAndNewlines)
            obj.start = start
            obj.end = end
            try? context.save()
            currentStart = nil
            isPlaying = false
            description = ""
            clearCurrentTracking()
        } else {
            // Start tracking
            currentStart = Date()
            isPlaying = true
            persistCurrentTracking()
        }
    }

    private func startTracking(from entry: TimeEntryEntity) {
        // If a session is active, finalize it first
        if isPlaying {
            let start = currentStart ?? Date()
            let end = Date()
            let current = TimeEntryEntity(context: context)
            current.id = UUID()
            current.title = description.trimmingCharacters(in: .whitespacesAndNewlines)
            current.start = start
            current.end = end
            try? context.save()
        }
        // Start a new session with the same title
        description = entry.title
        currentStart = Date()
        isPlaying = true
    }
    
    private func discardCurrent() {
        // Cancel current tracking without saving
        currentStart = nil
        isPlaying = false
        description = ""
        clearCurrentTracking()
    }
    
    private func deleteOffsets(_ offsets: IndexSet) {
        for index in offsets { context.delete(entries[index]) }
        try? context.save()
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

    private var matchingSuggestions: [String] {
        let query = description.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return [] }
        var seen = Set<String>()
        var results: [String] = []
        for entry in entries.sorted(by: { $0.start > $1.start }) {
            let title = entry.title.trimmingCharacters(in: .whitespacesAndNewlines)
            let key = title.lowercased()
            if title.isEmpty { continue }
            if seen.contains(key) { continue }
            if title.range(of: query, options: .caseInsensitive) != nil && key != query.lowercased() {
                results.append(title)
                seen.insert(key)
                if results.count >= 6 { break }
            }
        }
        return results
    }
    
    private func persistCurrentTracking() {
        guard let start = currentStart, isPlaying else {
            clearCurrentTracking()
            return
        }
        let ud = UserDefaults.standard
        ud.set(start, forKey: currentStartKey)
        ud.set(description, forKey: currentDescKey)
    }
    
    private func clearCurrentTracking() {
        let ud = UserDefaults.standard
        ud.removeObject(forKey: currentStartKey)
        ud.removeObject(forKey: currentDescKey)
    }
    
    private func loadCurrentTracking() {
        let ud = UserDefaults.standard
        if let start = ud.object(forKey: currentStartKey) as? Date {
            currentStart = min(start, Date())
            isPlaying = true
            description = ud.string(forKey: currentDescKey) ?? ""
        }
    }
}

#Preview {
    ContentView()
}
