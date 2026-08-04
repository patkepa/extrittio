import SwiftUI

struct JSONEditorView: View {
    @Binding var text: String
    let title: String
    let isEditable: Bool
    var disableSave: Bool = false
    var isSaving: Bool = false
    var onSave: ((String) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(title)
                    .font(.headline)
                Spacer()
                if isEditable, let onSave {
                    Button {
                        onSave(text)
                    } label: {
                        HStack(spacing: Spacing.xs) {
                            if isSaving {
                                ProgressView()
                                    .controlSize(.small)
                            }
                            Text(isSaving ? "Saving" : "Save")
                        }
                    }
                        .font(.callout.bold())
                        .disabled(disableSave || isSaving)
                }
            }

            TextEditor(text: isEditable ? $text : .constant(text))
                .font(.system(.caption, design: .monospaced))
                .frame(minHeight: 120)
                .scrollContentBackground(.hidden)
                .disabled(!isEditable)
        }
        .padding()
        .glassCard()
    }
}
