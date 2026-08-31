import CryptoKit
import Foundation
import Testing
@testable import Extrittio

@Suite("Device provisioning")
struct DeviceProvisioningTests {
    @Test("published blueprint exposes required transports")
    func blueprintTransports() throws {
        let json = Data(
            """
            {
              "id":"revision-1",
              "blueprint_id":"blueprint-1",
              "revision":3,
              "document":{"spec":{"transports":[
                {"binding":"primary","protocol":"zenoh"},
                {"binding":"fallback","protocol":"mqtt"}
              ],"configuration":{"type":"object"}}},
              "document_hash":"abc",
              "compatibility":{},
              "created_at":"2026-08-30T12:00:00Z"
            }
            """.utf8
        )

        let revision = try JSONDecoder().decode(DeviceBlueprintRevision.self, from: json)

        #expect(revision.transports == [
            DeviceBlueprintTransport(binding: "primary", protocolName: "zenoh"),
            DeviceBlueprintTransport(binding: "fallback", protocolName: "mqtt")
        ])
        #expect(revision.supportsConfiguration)
    }

    @Test("create request follows the blueprint device contract")
    func createRequestEncoding() throws {
        let request = CreateDeviceRequest(
            name: "Boiler room sensor",
            fleetId: 7,
            firmware: "1.2.3",
            blueprintRevisionId: "revision-1",
            configuration: .object(["sample_seconds": .int(30)])
        )
        let object = try #require(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as? [String: Any]
        )

        #expect(object["blueprint_revision_id"] as? String == "revision-1")
        #expect(object["fleet_id"] as? Int == 7)
        #expect(object["transport_bindings"] == nil)
    }

    @Test("bootstrap payload binds factory identity to inventory credentials")
    func payloadEncoding() throws {
        let payload = DeviceProvisioningPayload(
            factoryDeviceId: "factory-c6-001",
            device: testDevice,
            backend: try #require(
                DeviceProvisioningBackend(baseURL: URL(string: "http://192.168.4.20:8080/api/v1")!)
            ),
            thread: DeviceProvisioningThread(
                activeDatasetTLVs: "0e0800000000000100000003000019"
            ),
            contract: DeviceContract(
                id: "contract-1",
                deviceId: testDevice.id,
                blueprintRevisionId: "revision-1",
                contractHash: "hash",
                assignmentStatus: "pending",
                acknowledgedAt: nil,
                error: nil,
                createdAt: "2026-08-30T12:00:00Z",
                document: .object(["device_id": .string(testDevice.id)])
            ),
            certificate: DeviceCertificateResponse(
                certificatePem: "DEVICE CERT",
                privateKeyPem: "PRIVATE KEY",
                caPem: "CA CERT",
                fingerprint: "fingerprint",
                expiresAt: "2027-08-30T12:00:00Z",
                createdAt: "2026-08-30T12:00:00Z"
            )
        )

        let decoded = try JSONDecoder().decode(
            DeviceProvisioningPayload.self,
            from: payload.encoded()
        )

        #expect(decoded.version == DeviceProvisioningPayload.legacyProtocolVersion)
        #expect(decoded.factoryDeviceId == "factory-c6-001")
        #expect(decoded.deviceId == testDevice.id)
        #expect(decoded.backend.address == "192.168.4.20")
        #expect(decoded.backend.apiBaseURL == "http://192.168.4.20:8080/api/v1")
        #expect(decoded.thread?.activeDatasetTLVs == "0e0800000000000100000003000019")
        #expect(decoded.network == nil)
        #expect(decoded.credentials.privateKeyPem == "PRIVATE KEY")
    }

    @Test("Zephyr capabilities select the v4 network and contract envelope")
    func versionFourPayloadEncoding() throws {
        let capabilities = try JSONDecoder().decode(
            NearbyDeviceCapabilities.self,
            from: Data(
                #"{"bootstrap":[3,4],"network":["thread"],"transport":["zenoh-mtls"],"max_payload":20480}"#.utf8
            )
        )
        let info = NearbyDeviceInfo(
            deviceId: "factory-c6-001",
            model: "Zephyr device",
            firmwareVersion: "0.1.0",
            transport: "thread+zenoh-mtls",
            capabilities: capabilities
        )
        #expect(info.preferredBootstrapVersion == DeviceProvisioningPayload.currentProtocolVersion)

        let payload = DeviceProvisioningPayload(
            factoryDeviceId: info.deviceId,
            device: testDevice,
            backend: try #require(
                DeviceProvisioningBackend(baseURL: URL(string: "https://hub.example/api/v1")!)
            ),
            thread: DeviceProvisioningThread(
                activeDatasetTLVs: "0e0800000000000100000003000019"
            ),
            contract: DeviceContract(
                id: "contract-1",
                deviceId: testDevice.id,
                blueprintRevisionId: "revision-1",
                contractHash: String(repeating: "a", count: 64),
                assignmentStatus: "pending",
                acknowledgedAt: nil,
                error: nil,
                createdAt: "2026-08-30T12:00:00Z",
                document: .object(["deviceId": .string(testDevice.id)])
            ),
            certificate: DeviceCertificateResponse(
                certificatePem: "DEVICE CERT",
                privateKeyPem: "PRIVATE KEY",
                caPem: "CA CERT",
                fingerprint: "fingerprint",
                expiresAt: "2027-08-30T12:00:00Z",
                createdAt: "2026-08-30T12:00:00Z"
            ),
            protocolVersion: info.preferredBootstrapVersion
        )

        let object = try #require(
            JSONSerialization.jsonObject(with: payload.encoded()) as? [String: Any]
        )
        let network = try #require(object["network"] as? [String: Any])
        let contract = try #require(object["contract"] as? [String: Any])
        #expect(object["version"] as? Int == 4)
        #expect(object["thread"] == nil)
        #expect(network["type"] as? String == "thread")
        #expect(network["active_dataset_tlvs"] as? String == "0e0800000000000100000003000019")
        #expect(contract["contract_hash"] as? String == String(repeating: "a", count: 64))
        #expect(contract["document"] != nil)
    }

    @Test("Thread dataset response decodes the active operational dataset")
    func threadDatasetDecoding() throws {
        let response = Data(
            """
            {
              "active_dataset_tlvs":"0e0800000000000100000003000019",
              "network_key":"must-not-be-copied-separately",
              "pskc":"must-not-be-copied-separately"
            }
            """.utf8
        )

        let dataset = try JSONDecoder().decode(DeviceProvisioningThread.self, from: response)

        #expect(dataset.activeDatasetTLVs == "0e0800000000000100000003000019")
    }

    @Test("bootstrap backend preserves an IPv6 literal and usable URL")
    func ipv6BackendEncoding() throws {
        let backend = try #require(
            DeviceProvisioningBackend(baseURL: URL(string: "https://[fd12:3456::20]:8443/api/v1")!)
        )
        let object = try #require(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(backend)) as? [String: Any]
        )

        #expect(backend.address == "fd12:3456::20")
        #expect(object["address"] as? String == "fd12:3456::20")
        #expect(object["api_base_url"] as? String == "https://[fd12:3456::20]:8443/api/v1")
    }

    @Test("BLE provisioning frames are bounded and reconstruct the exact payload")
    func chunkedTransfer() throws {
        let payload = Data((0 ..< 600).map { UInt8($0 % 251) })
        let transfer = BLEProvisioningTransfer(payload: payload, transferId: 0x0102_0304)
        let frames = try transfer.frames(maximumWriteLength: 80)

        #expect(frames.allSatisfy { $0.data.count <= 80 })
        #expect(frames.first?.data.prefix(4) == Data([0x45, 0x58, 0x01, 0x01]))
        #expect(frames.last?.data.prefix(4) == Data([0x45, 0x58, 0x01, 0x03]))

        let dataFrames = frames.dropFirst().dropLast()
        let reconstructed = dataFrames.reduce(into: Data()) { result, frame in
            result.append(frame.data.dropFirst(12))
        }
        #expect(reconstructed == payload)
        #expect(frames.last?.data.dropFirst(8) == Data(SHA256.hash(data: payload)))
        #expect(frames.last?.completedPayloadBytes == payload.count)
    }

    @Test("BLE provisioning rejects an unusably small MTU")
    func smallMTU() {
        #expect(throws: BLEProvisioningTransferError.mtuTooSmall) {
            try BLEProvisioningTransfer(payload: Data([1])).frames(maximumWriteLength: 43)
        }
    }

    private var testDevice: Device {
        Device(
            id: "device-123",
            name: "Boiler room sensor",
            deviceTypeId: 1,
            deviceTypeName: "Sensor",
            fleetId: 7,
            fleetName: "Plant",
            status: "offline",
            firmware: "1.2.3",
            lastSeen: nil,
            lastSeenAt: nil,
            uptime: nil,
            uptimeSeconds: 0,
            latestLatitude: nil,
            latestLongitude: nil
        )
    }
}
