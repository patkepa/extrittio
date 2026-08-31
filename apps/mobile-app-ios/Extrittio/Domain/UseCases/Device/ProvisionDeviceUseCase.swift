import Foundation

struct ProvisionDeviceUseCase: Sendable {
    private let deviceRepository: any DeviceRepository
    private let certificateRepository: any CertificateRepository
    private let threadDatasetRepository: any ThreadDatasetRepository
    private let backendProvider: @Sendable () -> DeviceProvisioningBackend?

    init(
        deviceRepository: any DeviceRepository,
        certificateRepository: any CertificateRepository,
        threadDatasetRepository: any ThreadDatasetRepository,
        backendProvider: @escaping @Sendable () -> DeviceProvisioningBackend?
    ) {
        self.deviceRepository = deviceRepository
        self.certificateRepository = certificateRepository
        self.threadDatasetRepository = threadDatasetRepository
        self.backendProvider = backendProvider
    }

    func execute(
        request: CreateDeviceRequest,
        factoryDeviceId: String,
        bootstrapVersion: Int = DeviceProvisioningPayload.legacyProtocolVersion
    ) async throws -> PreparedDeviceProvisioning {
        guard let backend = backendProvider() else {
            throw ProvisionDeviceError.invalidBackendAddress
        }

        let device = try await deviceRepository.createDevice(request)
        do {
            // Download the one-time private key last, after all other server work succeeds.
            async let contractRequest = deviceRepository.getDeviceContract(id: device.id)
            async let threadRequest = threadDatasetRepository.getActiveDataset()
            let (contract, thread) = try await (contractRequest, threadRequest)
            let certificate = try await certificateRepository.getDeviceCertificate(deviceId: device.id)
            let payload = DeviceProvisioningPayload(
                factoryDeviceId: factoryDeviceId,
                device: device,
                backend: backend,
                thread: thread,
                contract: contract,
                certificate: certificate,
                protocolVersion: bootstrapVersion
            )
            return try PreparedDeviceProvisioning(device: device, payload: payload.encoded())
        } catch {
            // Compensate for a partially created inventory record when bootstrap data
            // could not be assembled. The caller has nothing usable to retry in that case.
            try? await deviceRepository.deleteDevice(id: device.id)
            throw error
        }
    }

    func rollback(deviceId: String) async throws {
        try await deviceRepository.deleteDevice(id: deviceId)
    }
}

enum ProvisionDeviceError: Error, LocalizedError, Equatable {
    case invalidBackendAddress

    var errorDescription: String? {
        switch self {
        case .invalidBackendAddress:
            "The configured server address cannot be included in the provisioning package."
        }
    }
}
