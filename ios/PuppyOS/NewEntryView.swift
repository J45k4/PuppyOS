import SwiftUI
import CoreData

struct NewEntryView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.managedObjectContext) private var context
    
    @State private var title: String = ""
    @State private var start: Date = Date()
    @State private var end: Date = Date()
    
    var body: some View {
        Form {
            Section(header: Text("Description")) {
                ZStack(alignment: .topLeading) {
                    if title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        Text("What are you doing?")
                            .foregroundColor(.secondary)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 8)
                            .allowsHitTesting(false)
                    }
                    TextEditor(text: $title)
                        .frame(minHeight: 96)
                        .font(.body)
                        .accessibilityLabel("Task description")
                }
            }
            Section(header: Text("Timing")) {
                DatePicker("Start", selection: $start, displayedComponents: [.date, .hourAndMinute])
                DatePicker("End", selection: $end, in: start...Date.distantFuture, displayedComponents: [.date, .hourAndMinute])
                HStack {
                    Text("Duration")
                    Spacer()
                    Text(formattedDuration(max(0, end.timeIntervalSince(start))))
                        .monospacedDigit()
                        .foregroundColor(.secondary)
                }
            }
        }
        .navigationTitle("New Entry")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Cancel") { dismiss() }
            }
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") { save() }
                    .disabled(end < start)
            }
        }
    }
    
    private func save() {
        let obj = TimeEntryEntity(context: context)
        obj.id = UUID()
        obj.title = title.trimmingCharacters(in: .whitespacesAndNewlines)
        obj.start = start
        obj.end = max(end, start)
        try? context.save()
        dismiss()
    }
    
    private func formattedDuration(_ interval: TimeInterval) -> String {
        let f = DateComponentsFormatter()
        f.allowedUnits = [.hour, .minute, .second]
        f.unitsStyle = .abbreviated
        f.zeroFormattingBehavior = .pad
        return f.string(from: interval) ?? "0s"
    }
}

