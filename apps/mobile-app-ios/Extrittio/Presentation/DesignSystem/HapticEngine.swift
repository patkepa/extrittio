// Extrittio/Presentation/DesignSystem/HapticEngine.swift
import UIKit

@MainActor
final class HapticEngine {
    static let shared = HapticEngine()

    private var notificationGenerator: UINotificationFeedbackGenerator?
    private var selectionGenerator: UISelectionFeedbackGenerator?
    private var impactGenerators: [UIImpactFeedbackGenerator.FeedbackStyle: UIImpactFeedbackGenerator] = [:]

    private init() {}

    func success() {
        let gen = ensureNotificationGenerator()
        gen.notificationOccurred(.success)
    }

    func error() {
        let gen = ensureNotificationGenerator()
        gen.notificationOccurred(.error)
    }

    func warning() {
        let gen = ensureNotificationGenerator()
        gen.notificationOccurred(.warning)
    }

    func selection() {
        let gen = ensureSelectionGenerator()
        gen.selectionChanged()
    }

    func impact(_ style: UIImpactFeedbackGenerator.FeedbackStyle = .light) {
        let gen = ensureImpactGenerator(style: style)
        gen.impactOccurred()
    }

    private func ensureNotificationGenerator() -> UINotificationFeedbackGenerator {
        if notificationGenerator == nil {
            notificationGenerator = UINotificationFeedbackGenerator()
            notificationGenerator?.prepare()
        }
        return notificationGenerator!
    }

    private func ensureSelectionGenerator() -> UISelectionFeedbackGenerator {
        if selectionGenerator == nil {
            selectionGenerator = UISelectionFeedbackGenerator()
            selectionGenerator?.prepare()
        }
        return selectionGenerator!
    }

    private func ensureImpactGenerator(style: UIImpactFeedbackGenerator.FeedbackStyle) -> UIImpactFeedbackGenerator {
        if impactGenerators[style] == nil {
            let gen = UIImpactFeedbackGenerator(style: style)
            gen.prepare()
            impactGenerators[style] = gen
        }
        return impactGenerators[style]!
    }
}
