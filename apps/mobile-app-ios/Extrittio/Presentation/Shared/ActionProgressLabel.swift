import SwiftUI

struct ActionProgressLabel: View {
    let title: String
    var systemImage: String? = nil
    var isLoading: Bool
    var loadingTitle: String? = nil

    var body: some View {
        HStack(spacing: Spacing.sm) {
            if isLoading {
                ProgressView()
                    .controlSize(.small)
            } else if let systemImage {
                Image(systemName: systemImage)
            }
            Text(isLoading ? (loadingTitle ?? title) : title)
        }
        .frame(maxWidth: .infinity)
    }
}
