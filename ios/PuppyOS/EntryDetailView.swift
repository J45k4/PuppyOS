import SwiftUI
import CoreData

struct EntryDetailView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.managedObjectContext) private var context
    @ObservedObject var entry: TimeEntryEntity
    var onDelete: (() -> Void)? = nil
    
    @State private var descriptionText: String = ""
    
    var body: some View {
        Form {
            Section(header: Text("Description")) {
                ZStack(alignment: .topLeading) {
                    if descriptionText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        Text("What are you doing?")
                            .foregroundColor(.secondary)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 8)
                            .allowsHitTesting(false)
                    }
                    TextEditor(text: $descriptionText)
                        .frame(minHeight: 96)
                        .font(.body)
                        .accessibilityLabel("Task description")
                }
            }
            
            Section(header: Text("Timing")) {
                DatePicker("Start", selection: Binding(get: { entry.start }, set: { entry.start = $0 }), displayedComponents: [.date, .hourAndMinute])
                DatePicker("End", selection: Binding(get: { entry.end }, set: { entry.end = max($0, entry.start) }), in: entry.start...Date.distantFuture, displayedComponents: [.date, .hourAndMinute])
                HStack {
                    Text("Duration")
                    Spacer()
                    Text(formattedDuration(max(0, entry.end.timeIntervalSince(entry.start))))
                        .monospacedDigit()
                        .foregroundColor(.secondary)
                }
            }
        }
        .navigationTitle("Edit Entry")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                if onDelete == nil { // treat as add-new flow; show Cancel
                    Button("Cancel") { dismiss() }
                }
            }
            ToolbarItem(placement: .destructiveAction) {
                if let onDelete {
                    Button(role: .destructive) {
                        onDelete()
                        dismiss()
                    } label: { Text("Delete") }
                }
            }
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") {
                    entry.title = descriptionText.trimmingCharacters(in: .whitespacesAndNewlines)
                    if entry.end < entry.start { entry.end = entry.start }
                    try? context.save()
                    dismiss()
                }
            }
        }
        .onAppear {
            descriptionText = entry.title
        }
    }
    
    private func formattedDuration(_ interval: TimeInterval) -> String {
        let f = DateComponentsFormatter()
        f.allowedUnits = [.hour, .minute, .second]
        f.unitsStyle = .abbreviated
        f.zeroFormattingBehavior = .pad
        return f.string(from: interval) ?? "0s"
    }
}
