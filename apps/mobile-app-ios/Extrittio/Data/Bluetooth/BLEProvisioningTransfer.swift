import CryptoKit
import Foundation

enum BLEProvisioningTransferError: Error, LocalizedError, Equatable {
    case emptyPayload
    case mtuTooSmall

    var errorDescription: String? {
        switch self {
        case .emptyPayload:
            "The provisioning payload is empty."
        case .mtuTooSmall:
            "The device Bluetooth connection cannot carry provisioning data."
        }
    }
}

struct BLEProvisioningFrame: Equatable, Sendable {
    let data: Data
    let completedPayloadBytes: Int
}

/// Binary protocol carried over the existing Extrittio message characteristic.
///
/// Every frame starts with `EX`, protocol version, opcode, and a four-byte transfer ID.
/// Start (`0x01`) adds payload length and SHA-256, data (`0x02`) adds byte offset and
/// raw bytes, and commit (`0x03`) repeats the digest. GATT writes-with-response provide
/// per-frame flow control; the peripheral validates the digest before applying commit.
/// The peripheral must require authenticated link encryption on this characteristic;
/// iOS will present the system pairing flow when the first protected write is attempted.
struct BLEProvisioningTransfer: Sendable {
    static let protocolVersion: UInt8 = 1
    static let startOpcode: UInt8 = 1
    static let dataOpcode: UInt8 = 2
    static let commitOpcode: UInt8 = 3
    static let minimumWriteLength = 44

    let payload: Data
    let transferId: UInt32

    init(payload: Data, transferId: UInt32 = UInt32.random(in: .min ... .max)) {
        self.payload = payload
        self.transferId = transferId
    }

    func frames(maximumWriteLength: Int) throws -> [BLEProvisioningFrame] {
        guard !payload.isEmpty else { throw BLEProvisioningTransferError.emptyPayload }
        guard maximumWriteLength >= Self.minimumWriteLength else {
            throw BLEProvisioningTransferError.mtuTooSmall
        }

        let digest = Data(SHA256.hash(data: payload))
        var start = header(opcode: Self.startOpcode)
        start.appendUInt32(UInt32(payload.count))
        start.append(digest)

        var result = [BLEProvisioningFrame(data: start, completedPayloadBytes: 0)]
        let chunkSize = maximumWriteLength - 12
        var offset = 0
        while offset < payload.count {
            let end = min(offset + chunkSize, payload.count)
            var frame = header(opcode: Self.dataOpcode)
            frame.appendUInt32(UInt32(offset))
            frame.append(payload[offset ..< end])
            result.append(BLEProvisioningFrame(data: frame, completedPayloadBytes: end))
            offset = end
        }

        var commit = header(opcode: Self.commitOpcode)
        commit.append(digest)
        result.append(BLEProvisioningFrame(data: commit, completedPayloadBytes: payload.count))
        return result
    }

    private func header(opcode: UInt8) -> Data {
        var data = Data([0x45, 0x58, Self.protocolVersion, opcode])
        data.appendUInt32(transferId)
        return data
    }
}

private extension Data {
    mutating func appendUInt32(_ value: UInt32) {
        var bigEndian = value.bigEndian
        Swift.withUnsafeBytes(of: &bigEndian) { append(contentsOf: $0) }
    }
}
