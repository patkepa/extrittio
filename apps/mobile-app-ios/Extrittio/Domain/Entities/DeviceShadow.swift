import Foundation

struct DeviceShadow: Codable, Sendable {
    let deviceId: String
    let desired: [String: AnyCodableValue]
    let reported: [String: AnyCodableValue]
    let delta: [String: AnyCodableValue]
    let version: Int
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case desired, reported, delta, version
        case deviceId = "device_id"
        case updatedAt = "updated_at"
    }
}

enum AnyCodableValue: Codable, Sendable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case object([String: AnyCodableValue])
    case array([AnyCodableValue])
    case null

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null; return }
        if let v = try? container.decode(Bool.self) { self = .bool(v) }
        else if let v = try? container.decode(Int.self) { self = .int(v) }
        else if let v = try? container.decode(Double.self) { self = .double(v) }
        else if let v = try? container.decode(String.self) { self = .string(v) }
        else if let v = try? container.decode([String: AnyCodableValue].self) { self = .object(v) }
        else if let v = try? container.decode([AnyCodableValue].self) { self = .array(v) }
        else { self = .null }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let v): try container.encode(v)
        case .int(let v): try container.encode(v)
        case .double(let v): try container.encode(v)
        case .bool(let v): try container.encode(v)
        case .object(let v): try container.encode(v)
        case .array(let v): try container.encode(v)
        case .null: try container.encodeNil()
        }
    }

    var displayString: String {
        switch self {
        case .string(let v): return v
        case .int(let v): return "\(v)"
        case .double(let v): return "\(v)"
        case .bool(let v): return "\(v)"
        case .object(let v):
            let pairs = v.map { "\($0.key): \($0.value.displayString)" }
            return "{\(pairs.joined(separator: ", "))}"
        case .array(let v):
            let items = v.map { $0.displayString }
            return "[\(items.joined(separator: ", "))]"
        case .null: return "null"
        }
    }
}

extension AnyCodableValue: Equatable {
    static func == (lhs: AnyCodableValue, rhs: AnyCodableValue) -> Bool {
        switch (lhs, rhs) {
        case (.string(let a), .string(let b)): a == b
        case (.int(let a), .int(let b)): a == b
        case (.double(let a), .double(let b)): a == b
        case (.bool(let a), .bool(let b)): a == b
        case (.null, .null): true
        case (.array(let a), .array(let b)): a == b
        case (.object(let a), .object(let b)): a == b
        default: false
        }
    }
}

extension AnyCodableValue: Hashable {
    func hash(into hasher: inout Hasher) {
        switch self {
        case .string(let v): hasher.combine(0); hasher.combine(v)
        case .int(let v): hasher.combine(1); hasher.combine(v)
        case .double(let v): hasher.combine(2); hasher.combine(v)
        case .bool(let v): hasher.combine(3); hasher.combine(v)
        case .null: hasher.combine(4)
        case .array(let v): hasher.combine(5); hasher.combine(v)
        case .object(let v):
            hasher.combine(6)
            for (key, val) in v.sorted(by: { $0.key < $1.key }) {
                hasher.combine(key)
                hasher.combine(val)
            }
        }
    }
}
