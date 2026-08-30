import Foundation
import Testing
@testable import Extrittio

@Suite("Nearby device contact pairing")
struct NearbyDeviceTests {
    @Test("RSSI is grouped into useful proximity bands", arguments: [
        (-82, NearbyDeviceProximity.detected),
        (-68, NearbyDeviceProximity.near),
        (-33, NearbyDeviceProximity.near),
        (-32, NearbyDeviceProximity.contact),
        (-20, NearbyDeviceProximity.contact)
    ])
    func proximity(rssi: Int, expected: NearbyDeviceProximity) {
        #expect(NearbyDeviceProximity(rssi: rssi) == expected)
    }

    @Test("contact threshold defaults to close-range pairing")
    func contactThreshold() {
        #expect(NearbyDeviceProximity.defaultContactRSSIThreshold == -32)
    }

    @Test("contact threshold can be customized")
    func customContactThreshold() {
        #expect(NearbyDeviceProximity(rssi: -40, contactRSSIThreshold: -42) == .contact)
        #expect(NearbyDeviceProximity(rssi: -40, contactRSSIThreshold: -35) == .near)
    }

    @Test("device identity uses the Extrittio device ID")
    func identity() {
        let info = NearbyDeviceInfo(
            deviceId: "esp32c6-thread-001",
            model: "ESP32-C6",
            firmwareVersion: "v1.0.0-esp32c6-thread-c",
            transport: "OpenThread + Zenoh"
        )

        #expect(info.id == "esp32c6-thread-001")
    }

    @Test("Double Socket identity enables relay controls")
    func doubleSocketIdentity() {
        let doubleSocket = NearbyDeviceInfo(
            deviceId: "double-socket",
            model: "Double Socket (ESP32-C6)",
            firmwareVersion: "26.7.3.3",
            transport: "Wi-Fi + Matter + Azure IoT"
        )
        let otherDevice = NearbyDeviceInfo(
            deviceId: "other",
            model: "ESP32-C6",
            firmwareVersion: "1.0.0",
            transport: "BLE"
        )

        #expect(doubleSocket.isDoubleSocket)
        #expect(!otherDevice.isDoubleSocket)
    }

    @Test("contact messages trim whitespace and encode as UTF-8")
    func contactMessage() {
        let message = NearbyDeviceContactMessage("  Hello, urządzenie!  ")

        #expect(message?.value == "Hello, urządzenie!")
        #expect(message?.data == Data("Hello, urządzenie!".utf8))
    }

    @Test("pairing commit uses the default contact payload")
    func pairingCommit() {
        #expect(NearbyDeviceContactMessage.pairingCommit.value == "Hello from Extrittio")
        #expect(NearbyDeviceContactMessage.pairingCommit.data == Data("Hello from Extrittio".utf8))
    }

    @Test("contact messages reject empty and oversized values")
    func invalidContactMessages() {
        #expect(NearbyDeviceContactMessage("   ") == nil)
        #expect(
            NearbyDeviceContactMessage(
                String(repeating: "a", count: NearbyDeviceContactMessage.maximumUTF8Length + 1)
            ) == nil
        )
    }

    @Test("relay commands match the Double Socket BLE JSON contract", arguments: [
        (NearbyDeviceRelayTarget.left, NearbyDeviceRelayState.on,
         "{\"v\":1,\"cmd\":\"relay\",\"gang\":\"left\",\"state\":1}"),
        (NearbyDeviceRelayTarget.right, NearbyDeviceRelayState.off,
         "{\"v\":1,\"cmd\":\"relay\",\"gang\":\"right\",\"state\":0}"),
        (NearbyDeviceRelayTarget.all, NearbyDeviceRelayState.on,
         "{\"v\":1,\"cmd\":\"relay\",\"gang\":\"all\",\"state\":1}")
    ])
    func relayCommand(
        target: NearbyDeviceRelayTarget,
        state: NearbyDeviceRelayState,
        expectedJSON: String
    ) {
        let command = NearbyDeviceCommand.relay(target: target, state: state)

        #expect(command.data == Data(expectedJSON.utf8))
        #expect(command.data.count <= NearbyDeviceContactMessage.maximumUTF8Length)
    }

    @Test("sound commands match the Double Socket BLE JSON contract", arguments: [
        (NearbyDeviceSound.locate, 1, "{\"v\":1,\"cmd\":\"sound\",\"type\":\"locate\",\"repeat\":1}"),
        (NearbyDeviceSound.doorbell, 5, "{\"v\":1,\"cmd\":\"sound\",\"type\":\"doorbell\",\"repeat\":5}"),
        (NearbyDeviceSound.other, 10, "{\"v\":1,\"cmd\":\"sound\",\"type\":\"other\",\"repeat\":10}")
    ])
    func soundCommand(type: NearbyDeviceSound, repeatCount: Int, expectedJSON: String) {
        let command = NearbyDeviceCommand.sound(type: type, repeatCount: repeatCount)

        #expect(command?.data == Data(expectedJSON.utf8))
        #expect((command?.data.count ?? Int.max) <= NearbyDeviceContactMessage.maximumUTF8Length)
    }

    @Test("sound commands reject repeat counts outside the firmware range", arguments: [0, 11])
    func invalidSoundRepeatCount(repeatCount: Int) {
        #expect(NearbyDeviceCommand.sound(type: .locate, repeatCount: repeatCount) == nil)
    }
}
