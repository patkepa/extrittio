// Extrittio/Presentation/DesignSystem/AnimationConstants.swift
import SwiftUI

enum AppAnimation {
    case standard
    case quick
    case gentle
    case micro
    case chart

    var animation: Animation {
        switch self {
        case .standard: .spring(response: 0.5, dampingFraction: 0.8)
        case .quick: .spring(response: 0.4, dampingFraction: 0.85)
        case .gentle: .spring(response: 0.6, dampingFraction: 0.75)
        case .micro: .easeOut(duration: 0.15)
        case .chart: .spring(response: 0.55, dampingFraction: 0.7)
        }
    }

    var duration: Double {
        switch self {
        case .standard: 0.3
        case .quick: 0.2
        case .gentle: 0.5
        case .micro: 0.15
        case .chart: 0.6
        }
    }
}
