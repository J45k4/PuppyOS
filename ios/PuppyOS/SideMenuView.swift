import SwiftUI

enum SideMenuItem: Hashable {
    case track
    case timers
    case files
    case photos
}

struct SideMenuView: View {
    var onSelect: (SideMenuItem) -> Void
    
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Image(systemName: "pawprint.fill")
                    .foregroundColor(.blue)
                Text("PuppyOS")
                    .font(.headline)
                    .foregroundColor(.primary)
            }
            .padding(.horizontal)
            .padding(.top, 16)
            .padding(.bottom, 12)
            Divider()
            Button { onSelect(.track) } label: {
                Label("Track", systemImage: "clock.arrow.circlepath")
                    .padding(.horizontal)
                    .padding(.vertical, 12)
            }
            .buttonStyle(.plain)
            Button { onSelect(.timers) } label: {
                Label("Timers", systemImage: "timer")
                    .padding(.horizontal)
                    .padding(.vertical, 12)
            }
            .buttonStyle(.plain)
            Button { onSelect(.files) } label: {
                Label("Files", systemImage: "folder")
                    .padding(.horizontal)
                    .padding(.vertical, 12)
            }
            .buttonStyle(.plain)
            Button { onSelect(.photos) } label: {
                Label("Photos", systemImage: "photo.on.rectangle")
                    .padding(.horizontal)
                    .padding(.vertical, 12)
            }
            .buttonStyle(.plain)
            Spacer()
            Text("v1.0")
                .font(.footnote)
                .foregroundColor(.secondary)
                .padding(.horizontal)
                .padding(.bottom, 20)
        }
        .frame(maxHeight: .infinity)
        .frame(width: 280)
        .background(.ultraThickMaterial)
    }
}
