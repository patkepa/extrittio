import Foundation

struct DeviceBlueprint: Codable, Sendable, Identifiable, Equatable {
    let id: String
    let key: String
    let name: String
    let description: String?
    let latestRevision: Int?
    let createdAt: String
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id, key, name, description
        case latestRevision = "latest_revision"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

struct DeviceBlueprintRevision: Codable, Sendable, Identifiable, Equatable {
    let id: String
    let blueprintId: String
    let revision: Int
    let document: AnyCodableValue
    let documentHash: String
    let compatibility: AnyCodableValue
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id, revision, document, compatibility
        case blueprintId = "blueprint_id"
        case documentHash = "document_hash"
        case createdAt = "created_at"
    }

    var transports: [DeviceBlueprintTransport] {
        guard case .object(let root) = document,
              case .object(let spec)? = root["spec"],
              case .array(let transports)? = spec["transports"]
        else { return [] }

        return transports.compactMap { value in
            guard case .object(let transport) = value,
                  case .string(let binding)? = transport["binding"],
                  case .string(let protocolName)? = transport["protocol"]
            else { return nil }
            return DeviceBlueprintTransport(binding: binding, protocolName: protocolName)
        }
    }

    var supportsConfiguration: Bool {
        guard case .object(let root) = document,
              case .object(let spec)? = root["spec"]
        else { return false }
        return spec["configuration"] != nil
    }
}

struct DeviceBlueprintTransport: Sendable, Identifiable, Equatable {
    let binding: String
    let protocolName: String
    var id: String { binding }
}

struct DeviceContract: Codable, Sendable, Equatable {
    let id: String
    let deviceId: String
    let blueprintRevisionId: String
    let contractHash: String
    let assignmentStatus: String
    let acknowledgedAt: String?
    let error: String?
    let createdAt: String
    let document: AnyCodableValue

    enum CodingKeys: String, CodingKey {
        case id, error, document
        case deviceId = "device_id"
        case blueprintRevisionId = "blueprint_revision_id"
        case contractHash = "contract_hash"
        case assignmentStatus = "assignment_status"
        case acknowledgedAt = "acknowledged_at"
        case createdAt = "created_at"
    }
}

struct DeviceProvisioningPayload: Codable, Sendable, Equatable {
    static let protocolVersion = 3

    let version: Int
    let factoryDeviceId: String
    let deviceId: String
    let deviceName: String
    let backend: DeviceProvisioningBackend
    let thread: DeviceProvisioningThread
    let contract: AnyCodableValue
    let credentials: DeviceProvisioningCredentials

    init(
        factoryDeviceId: String,
        device: Device,
        backend: DeviceProvisioningBackend,
        thread: DeviceProvisioningThread,
        contract: DeviceContract,
        certificate: DeviceCertificateResponse
    ) {
        self.version = Self.protocolVersion
        self.factoryDeviceId = factoryDeviceId
        self.deviceId = device.id
        self.deviceName = device.name
        self.backend = backend
        self.thread = thread
        self.contract = contract.document
        self.credentials = DeviceProvisioningCredentials(certificate: certificate)
    }

    enum CodingKeys: String, CodingKey {
        case version
        case factoryDeviceId = "factory_device_id"
        case deviceId = "device_id"
        case deviceName = "device_name"
        case backend, thread, contract, credentials
    }

    func encoded() throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(self)
    }
}

struct DeviceProvisioningThread: Codable, Sendable, Equatable {
    let activeDatasetTLVs: String

    enum CodingKeys: String, CodingKey {
        case activeDatasetTLVs = "active_dataset_tlvs"
    }
}

struct DeviceProvisioningBackend: Codable, Sendable, Equatable {
    let address: String
    let apiBaseURL: String

    init?(baseURL: URL) {
        guard let components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false),
              let address = components.host,
              !address.isEmpty
        else { return nil }

        if address.hasPrefix("["), address.hasSuffix("]") {
            self.address = String(address.dropFirst().dropLast())
        } else {
            self.address = address
        }
        self.apiBaseURL = baseURL.absoluteString
    }

    enum CodingKeys: String, CodingKey {
        case address
        case apiBaseURL = "api_base_url"
    }
}

struct DeviceProvisioningCredentials: Codable, Sendable, Equatable {
    let certificatePem: String
    let privateKeyPem: String
    let caPem: String
    let fingerprint: String

    init(certificate: DeviceCertificateResponse) {
        self.certificatePem = certificate.certificatePem
        self.privateKeyPem = certificate.privateKeyPem
        self.caPem = certificate.caPem
        self.fingerprint = certificate.fingerprint
    }

    enum CodingKeys: String, CodingKey {
        case fingerprint
        case certificatePem = "certificate_pem"
        case privateKeyPem = "private_key_pem"
        case caPem = "ca_pem"
    }
}

struct PreparedDeviceProvisioning: Sendable {
    let device: Device
    let payload: Data
}
