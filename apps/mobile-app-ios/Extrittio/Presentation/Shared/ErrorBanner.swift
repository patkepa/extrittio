import SwiftUI

struct ErrorBanner: View {
    let message: String
    var retryAction: (() -> Void)?

    var body: some View {
        HStack {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.orange)
            Text(message)
                .font(.callout)
            Spacer()
            if let retryAction {
                Button("Retry", action: retryAction)
                    .font(.callout.bold())
            }
        }
        .padding()
        .glassCard()
        .padding(.horizontal)
    }
}
