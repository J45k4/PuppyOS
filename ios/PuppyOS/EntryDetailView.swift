import SwiftUI

struct EntryDetailView: View {
    @Environment(\.dismiss) private var dismiss
    @State var entry: TimeEntry
    var onSave: (TimeEntry) -> Void
    
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
                DatePicker("Start", selection: $entry.start, displayedComponents: [.date, .hourAndMinute])
                DatePicker("End", selection: $entry.end, in: entry.start...Date.distantFuture, displayedComponents: [.date, .hourAndMinute])
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
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") {
                    var updated = entry
                    updated.title = descriptionText.trimmingCharacters(in: .whitespacesAndNewlines)
                    if updated.end < updated.start { updated.end = updated.start }
                    onSave(updated)
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
