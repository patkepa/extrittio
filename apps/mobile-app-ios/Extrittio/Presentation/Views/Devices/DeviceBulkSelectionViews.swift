import SwiftUI

enum DeviceBulkAction: String, Identifiable, Sendable {
    case restart
    case delete

    var id: String { rawValue }

    var confirmationTitle: String {
        switch self {
        case .restart: "Restart Devices?"
        case .delete: "Delete Devices?"
        }
    }

    var confirmationButtonTitle: String {
        switch self {
        case .restart: "Restart"
        case .delete: "Delete"
        }
    }

    var systemImage: String {
        switch self {
        case .restart: "arrow.clockwise"
        case .delete: "trash"
        }
    }

    func confirmationMessage(count: Int) -> String {
        switch self {
        case .restart:
            "Restart \(deviceCountText(count))."
        case .delete:
            "Delete \(deviceCountText(count)). This cannot be undone."
        }
    }

    func toast(for result: DeviceBulkActionResult) -> ToastType {
        if result.succeededCount == 0 {
            switch self {
            case .restart: return .error("Restart failed")
            case .delete: return .error("Delete failed")
            }
        }

        if result.failedCount > 0 {
            switch self {
            case .restart:
                return .warning("Restarted \(result.succeededCount) of \(result.requestedCount) devices")
            case .delete:
                return .warning("Deleted \(result.succeededCount) of \(result.requestedCount) devices")
            }
        }

        switch self {
        case .restart:
            return .success("Restarting \(deviceCountText(result.succeededCount))")
        case .delete:
            return .success("Deleted \(deviceCountText(result.succeededCount))")
        }
    }

    private func deviceCountText(_ count: Int) -> String {
        "\(count) \(count == 1 ? "device" : "devices")"
    }
}

struct DeviceBulkActionBar: View {
    let selectedCount: Int
    let visibleCount: Int
    let allVisibleSelected: Bool
    let isOnline: Bool
    let isWorking: Bool
    let toggleVisibleSelection: () -> Void
    let restartSelected: () -> Void
    let deleteSelected: () -> Void

    var body: some View {
        VStack(spacing: Spacing.sm) {
            Divider()

            HStack {
                Text("\(selectedCount) selected")
                    .font(.subheadline.weight(.semibold))
                    .lineLimit(1)

                Spacer()

                Button {
                    toggleVisibleSelection()
                } label: {
                    Label(allVisibleSelected ? "Clear Visible" : "Select Visible", systemImage: allVisibleSelected ? "minus.circle" : "checkmark.circle")
                }
                .font(.caption.weight(.semibold))
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(visibleCount == 0 || isWorking)
            }

            HStack(spacing: Spacing.sm) {
                Button {
                    restartSelected()
                } label: {
                    Label("Restart", systemImage: DeviceBulkAction.restart.systemImage)
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .disabled(selectedCount == 0 || !isOnline || isWorking)

                Button(role: .destructive) {
                    deleteSelected()
                } label: {
                    Label("Delete", systemImage: DeviceBulkAction.delete.systemImage)
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .disabled(selectedCount == 0 || !isOnline || isWorking)
            }
            .controlSize(.small)
        }
        .padding(.horizontal, Spacing.md)
        .padding(.top, Spacing.xs)
        .padding(.bottom, Spacing.md)
        .background(.regularMaterial)
    }
}

struct SelectableDeviceRow: View {
    let device: Device
    let isSelected: Bool
    let isDimmed: Bool

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                .font(.title3)
                .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
                .frame(width: 28, height: 48)
                .padding(.top, Spacing.md)

            DeviceRowView(device: device)
        }
        .contentShape(Rectangle())
        .opacity(isDimmed ? 0.6 : 1)
    }
}
