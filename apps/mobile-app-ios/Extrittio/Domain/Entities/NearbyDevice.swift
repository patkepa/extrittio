import Foundation

struct NearbyDeviceInfo: Equatable, Identifiable, Sendable {
    let deviceId: String
    let model: String
    let firmwareVersion: String
    let transport: String

    var id: String { deviceId }

    var isDoubleSocket: Bool {
        model.range(of: "Double Socket", options: .caseInsensitive) != nil
    }
}

struct NearbyDeviceCandidate: Equatable, Identifiable, Sendable {
    let id: String
    let displayName: String
    let rssi: Int

    var proximity: NearbyDeviceProximity {
        proximity(contactRSSIThreshold: NearbyDeviceProximity.defaultContactRSSIThreshold)
    }

    func proximity(contactRSSIThreshold: Int) -> NearbyDeviceProximity {
        NearbyDeviceProximity(rssi: rssi, contactRSSIThreshold: contactRSSIThreshold)
    }
}

struct NearbyDeviceContactMessage: Equatable, Sendable {
    static let maximumUTF8Length = 120
    static let pairingCommit = NearbyDeviceContactMessage(validatedValue: "Hello from Extrittio")

    let value: String

    init?(_ input: String) {
        let value = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty, value.utf8.count <= Self.maximumUTF8Length else {
            return nil
        }
        self.value = value
    }

    var data: Data { Data(value.utf8) }

    private init(validatedValue: String) {
        value = validatedValue
    }
}

enum NearbyDeviceRelayTarget: String, CaseIterable, Identifiable, Sendable {
    case left
    case right
    case all

    var id: String { rawValue }

    var title: String {
        switch self {
        case .left: "Left"
        case .right: "Right"
        case .all: "Both"
        }
    }

    var commandDescription: String {
        switch self {
        case .left: "left outlet"
        case .right: "right outlet"
        case .all: "both outlets"
        }
    }
}

enum NearbyDeviceRelayState: Int, Sendable {
    case off = 0
    case on = 1

    var commandDescription: String {
        switch self {
        case .off: "Turn off"
        case .on: "Turn on"
        }
    }
}

enum NearbyDeviceSound: String, CaseIterable, Identifiable, Sendable {
    case locate
    case doorbell
    case other

    var id: String { rawValue }

    var title: String {
        switch self {
        case .locate: "Locate"
        case .doorbell: "Doorbell"
        case .other: "Other"
        }
    }
}

struct NearbyDeviceCommand: Equatable, Sendable {
    static let supportedSoundRepeatCounts = 1 ... 10

    private enum Payload: Equatable, Sendable {
        case relay(target: NearbyDeviceRelayTarget, state: NearbyDeviceRelayState)
        case sound(type: NearbyDeviceSound, repeatCount: Int)
    }

    private let payload: Payload

    static func relay(
        target: NearbyDeviceRelayTarget,
        state: NearbyDeviceRelayState
    ) -> NearbyDeviceCommand {
        NearbyDeviceCommand(payload: .relay(target: target, state: state))
    }

    static func sound(
        type: NearbyDeviceSound,
        repeatCount: Int
    ) -> NearbyDeviceCommand? {
        guard supportedSoundRepeatCounts.contains(repeatCount) else { return nil }
        return NearbyDeviceCommand(payload: .sound(type: type, repeatCount: repeatCount))
    }

    var data: Data {
        let json = switch payload {
        case .relay(let target, let state):
            "{\"v\":1,\"cmd\":\"relay\",\"gang\":\"\(target.rawValue)\",\"state\":\(state.rawValue)}"
        case .sound(let type, let repeatCount):
            "{\"v\":1,\"cmd\":\"sound\",\"type\":\"\(type.rawValue)\",\"repeat\":\(repeatCount)}"
        }
        return Data(json.utf8)
    }

    var progressMessage: String {
        switch payload {
        case .relay(let target, let state):
            "Sending \(state.commandDescription.lowercased()) for \(target.commandDescription)…"
        case .sound(let type, let repeatCount):
            "Sending \(type.title.lowercased()) sound ×\(repeatCount)…"
        }
    }

    var successMessage: String {
        switch payload {
        case .relay(let target, let state):
            "\(state.commandDescription) command accepted for \(target.commandDescription)."
        case .sound(let type, let repeatCount):
            "\(type.title) sound command accepted ×\(repeatCount)."
        }
    }
}

enum NearbyDeviceWriteRequest: Equatable, Sendable {
    case pairingCommit
    case command(NearbyDeviceCommand)

    var data: Data {
        switch self {
        case .pairingCommit: NearbyDeviceContactMessage.pairingCommit.data
        case .command(let command): command.data
        }
    }

    var progressMessage: String {
        switch self {
        case .pairingCommit: "Confirming close-range connection…"
        case .command(let command): command.progressMessage
        }
    }

    var successMessage: String {
        switch self {
        case .pairingCommit: "Connected. The device flashed green three times."
        case .command(let command): command.successMessage
        }
    }
}

enum NearbyDeviceProximity: Equatable, Sendable {
    static let defaultContactRSSIThreshold = -32
    static let supportedContactRSSIThresholds = -80 ... -20

    case detected
    case near
    case contact

    init(
        rssi: Int,
        contactRSSIThreshold: Int = Self.defaultContactRSSIThreshold
    ) {
        if rssi >= contactRSSIThreshold {
            self = .contact
        } else if rssi >= -68 {
            self = .near
        } else {
            self = .detected
        }
    }

    var progress: Double {
        return switch self {
        case .detected: 0.25
        case .near: 0.65
        case .contact: 1
        }
    }
}

enum NearbyDeviceAvailability: Equatable, Sendable {
    case bluetoothOff
    case unauthorized
    case unsupported
}

enum NearbyDeviceScanState: Equatable, Sendable {
    case idle
    case preparing
    case scanning(NearbyDeviceCandidate?)
    case connecting(NearbyDeviceCandidate)
    case reading(NearbyDeviceCandidate)
    case identified(NearbyDeviceInfo, NearbyDeviceCandidate)
    case unavailable(NearbyDeviceAvailability)
    case failed(String)

    var isSearching: Bool {
        switch self {
        case .preparing, .scanning, .connecting, .reading:
            true
        case .idle, .identified, .unavailable, .failed:
            false
        }
    }
}

enum NearbyDeviceWriteState: Equatable, Sendable {
    case idle
    case writing(NearbyDeviceWriteRequest)
    case succeeded(NearbyDeviceWriteRequest)
    case failed(NearbyDeviceWriteRequest?, String)

    var isWriting: Bool {
        if case .writing = self {
            return true
        }
        return false
    }
}

enum NearbyDeviceProvisioningState: Equatable, Sendable {
    case idle
    case transferring(completedBytes: Int, totalBytes: Int)
    case succeeded
    case failed(String)

    var progress: Double {
        switch self {
        case .idle:
            return 0
        case .transferring(let completedBytes, let totalBytes):
            guard totalBytes > 0 else { return 0 }
            return min(Double(completedBytes) / Double(totalBytes), 1)
        case .succeeded:
            return 1
        case .failed:
            return 0
        }
    }

    var isTransferring: Bool {
        if case .transferring = self { return true }
        return false
    }
}
