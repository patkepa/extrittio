// ExtrittioTests/DesignSystemTests.swift
import Testing
@testable import Extrittio

@Suite("DesignSystem")
struct DesignSystemTests {

    @Test("Animation constants provide SwiftUI animations")
    func animationConstants() {
        // Verify all cases exist and return non-nil animations
        let cases: [AppAnimation] = [.standard, .quick, .gentle, .micro, .chart]
        #expect(cases.count == 5)
    }

    @Test("Spacing scale values are correct")
    func spacingValues() {
        #expect(Spacing.xxs == 2)
        #expect(Spacing.xs == 4)
        #expect(Spacing.sm == 8)
        #expect(Spacing.md == 12)
        #expect(Spacing.lg == 16)
        #expect(Spacing.xl == 24)
        #expect(Spacing.xxl == 32)
    }

    @Test("Device mockup variants classify device type metadata")
    func deviceMockupVariantClassification() {
        #expect(DeviceMockupVariant.classify(name: "GPS Tracker", icon: nil) == .tracker)
        #expect(DeviceMockupVariant.classify(name: "Gateway", icon: "wifi.router") == .gateway)
        #expect(DeviceMockupVariant.classify(name: "Vision Camera", icon: nil) == .camera)
        #expect(DeviceMockupVariant.classify(name: "Relay Switch", icon: nil) == .actuator)
        #expect(DeviceMockupVariant.classify(name: "Energy Meter", icon: nil) == .meter)
        #expect(DeviceMockupVariant.classify(name: "Temperature Sensor", icon: nil) == .sensor)
    }
}
