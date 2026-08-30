@preconcurrency import CoreBluetooth
import Foundation
import Observation
import os

@Observable
@MainActor
final class NearbyDeviceScanner: NSObject {
    private(set) var state: NearbyDeviceScanState = .idle
    private(set) var writeState: NearbyDeviceWriteState = .idle
    private(set) var provisioningState: NearbyDeviceProvisioningState = .idle
    private(set) var contactRSSIThreshold = NearbyDeviceProximity.defaultContactRSSIThreshold
    private(set) var nearbyDevices: [NearbyDeviceCandidate] = []

    @ObservationIgnored private var centralManager: CBCentralManager!
    @ObservationIgnored private var selectedPeripheral: CBPeripheral?
    @ObservationIgnored private var messageCharacteristic: CBCharacteristic?
    @ObservationIgnored private var peripherals: [UUID: CBPeripheral] = [:]
    @ObservationIgnored private var candidates: [UUID: NearbyDeviceCandidate] = [:]
    @ObservationIgnored private var closeSampleCounts: [UUID: Int] = [:]
    @ObservationIgnored private var pendingCharacteristicUUIDs = Set<CBUUID>()
    @ObservationIgnored private var characteristicValues: [CBUUID: String] = [:]
    @ObservationIgnored private var scanningRequested = false
    @ObservationIgnored private var didAttemptInitialContactWrite = false
    @ObservationIgnored private var provisioningFrames: [BLEProvisioningFrame] = []
    @ObservationIgnored private var currentProvisioningFrame: BLEProvisioningFrame?
    @ObservationIgnored private var provisioningPayloadSize = 0
    @ObservationIgnored private let sendsIdentificationFeedback: Bool
    @ObservationIgnored private let logger = Logger(subsystem: "com.extrittio", category: "NearbyDeviceScanner")

    init(sendsIdentificationFeedback: Bool = true) {
        self.sendsIdentificationFeedback = sendsIdentificationFeedback
        super.init()
        centralManager = CBCentralManager(delegate: self, queue: .main)
    }

    func startScanning() {
        scanningRequested = true
        centralManager.stopScan()
        cancelSelectedConnection()
        peripherals.removeAll()
        candidates.removeAll()
        nearbyDevices = []
        closeSampleCounts.removeAll()
        characteristicValues.removeAll()
        pendingCharacteristicUUIDs.removeAll()
        messageCharacteristic = nil
        didAttemptInitialContactWrite = false
        writeState = .idle
        resetProvisioning()
        state = .preparing
        startScanningIfAvailable()
    }

    func stopScanning() {
        scanningRequested = false
        centralManager.stopScan()
        cancelSelectedConnection()
        messageCharacteristic = nil
        writeState = .idle
        resetProvisioning()
        state = .idle
    }

    func setContactRSSIThreshold(_ threshold: Int) {
        let supportedThresholds = NearbyDeviceProximity.supportedContactRSSIThresholds
        let constrainedThreshold = min(max(threshold, supportedThresholds.lowerBound), supportedThresholds.upperBound)
        guard constrainedThreshold != contactRSSIThreshold else { return }

        contactRSSIThreshold = constrainedThreshold
        closeSampleCounts.removeAll()
    }

    func requestIdentificationFeedback() {
        write(.pairingCommit)
    }

    func send(_ command: NearbyDeviceCommand) {
        write(.command(command))
    }

    func provision(payload: Data) {
        guard !writeState.isWriting, !provisioningState.isTransferring else { return }
        guard
            case .identified = state,
            let selectedPeripheral,
            selectedPeripheral.state == .connected,
            let messageCharacteristic
        else {
            provisioningState = .failed("The nearby device is no longer connected.")
            return
        }

        do {
            provisioningFrames = try BLEProvisioningTransfer(payload: payload).frames(
                maximumWriteLength: selectedPeripheral.maximumWriteValueLength(for: .withResponse)
            )
            provisioningPayloadSize = payload.count
            provisioningState = .transferring(completedBytes: 0, totalBytes: payload.count)
            writeNextProvisioningFrame(to: selectedPeripheral, characteristic: messageCharacteristic)
        } catch {
            provisioningState = .failed(error.localizedDescription)
        }
    }

    /// Connects to an explicitly selected device without requiring contact-range RSSI.
    func connect(to candidate: NearbyDeviceCandidate) {
        guard case .scanning = state,
              let peripheralID = UUID(uuidString: candidate.id),
              let peripheral = peripherals[peripheralID]
        else { return }

        connect(to: peripheral, candidate: candidate)
    }

    private func write(_ request: NearbyDeviceWriteRequest) {
        guard !writeState.isWriting else { return }
        guard
            case .identified = state,
            let selectedPeripheral,
            selectedPeripheral.state == .connected,
            let messageCharacteristic
        else {
            writeState = .failed(request, "The nearby device is no longer connected.")
            return
        }

        guard request.data.count <= NearbyDeviceContactMessage.maximumUTF8Length,
              request.data.count <= selectedPeripheral.maximumWriteValueLength(for: .withResponse)
        else {
            writeState = .failed(request, "The request is too large for this Bluetooth connection.")
            return
        }

        writeState = .writing(request)
        selectedPeripheral.writeValue(request.data, for: messageCharacteristic, type: .withResponse)
    }

    private func startScanningIfAvailable() {
        guard scanningRequested else { return }

        switch centralManager.state {
        case .poweredOn:
            state = .scanning(nil)
            centralManager.scanForPeripherals(
                withServices: [BLEContract.service],
                options: [CBCentralManagerScanOptionAllowDuplicatesKey: true]
            )
        case .poweredOff:
            state = .unavailable(.bluetoothOff)
        case .unauthorized:
            state = .unavailable(.unauthorized)
        case .unsupported:
            state = .unavailable(.unsupported)
        case .resetting, .unknown:
            state = .preparing
        @unknown default:
            state = .unavailable(.unsupported)
        }
    }

    private func recordDiscovery(
        peripheral: CBPeripheral,
        advertisementData: [String: Any],
        rssi: Int
    ) {
        guard rssi != 127 else { return }

        let identifier = peripheral.identifier
        let previousRSSI = candidates[identifier]?.rssi
        let smoothedRSSI = previousRSSI.map {
            Int((Double($0) * 0.65 + Double(rssi) * 0.35).rounded())
        } ?? rssi
        let advertisedName = advertisementData[CBAdvertisementDataLocalNameKey] as? String
        let candidate = NearbyDeviceCandidate(
            id: identifier.uuidString,
            displayName: advertisedName ?? peripheral.name ?? "Extrittio device",
            rssi: smoothedRSSI
        )

        peripherals[identifier] = peripheral
        candidates[identifier] = candidate
        nearbyDevices = candidates.values.sorted {
            if $0.rssi == $1.rssi {
                return $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
            }
            return $0.rssi > $1.rssi
        }
        if candidate.proximity(contactRSSIThreshold: contactRSSIThreshold) == .contact {
            closeSampleCounts[identifier, default: 0] += 1
        } else {
            closeSampleCounts[identifier] = 0
        }

        let closest = candidates.values.max { $0.rssi < $1.rssi }
        state = .scanning(closest)

        guard closeSampleCounts[identifier, default: 0] >= 2 else { return }
        connect(to: peripheral, candidate: candidate)
    }

    private func connect(to peripheral: CBPeripheral, candidate: NearbyDeviceCandidate) {
        centralManager.stopScan()
        selectedPeripheral = peripheral
        state = .connecting(candidate)
        centralManager.connect(peripheral)
    }

    private func fail(_ message: String) {
        logger.error("BLE contact pairing failed: \(message, privacy: .public)")
        if provisioningState.isTransferring {
            provisioningState = .failed(message)
        }
        cancelSelectedConnection()
        state = .failed(message)
    }

    private func cancelSelectedConnection() {
        guard let selectedPeripheral else { return }
        self.selectedPeripheral = nil
        selectedPeripheral.delegate = nil
        centralManager.cancelPeripheralConnection(selectedPeripheral)
    }

    private func completeIdentityRead(candidate: NearbyDeviceCandidate) {
        guard
            let deviceId = characteristicValues[BLEContract.deviceId],
            let model = characteristicValues[BLEContract.model],
            let firmware = characteristicValues[BLEContract.firmware],
            let transport = characteristicValues[BLEContract.transport]
        else {
            fail("The device returned incomplete identity information.")
            return
        }

        let info = NearbyDeviceInfo(
            deviceId: deviceId,
            model: model,
            firmwareVersion: firmware,
            transport: transport
        )
        state = .identified(info, candidate)
        writeState = .idle
        writeInitialContactMessageIfNeeded()
    }

    private func writeInitialContactMessageIfNeeded() {
        guard sendsIdentificationFeedback else { return }
        guard !didAttemptInitialContactWrite else { return }
        didAttemptInitialContactWrite = true
        write(.pairingCommit)
    }

    private func writeNextProvisioningFrame(
        to peripheral: CBPeripheral,
        characteristic: CBCharacteristic
    ) {
        guard !provisioningFrames.isEmpty else {
            currentProvisioningFrame = nil
            provisioningState = .succeeded
            return
        }
        let frame = provisioningFrames.removeFirst()
        currentProvisioningFrame = frame
        peripheral.writeValue(frame.data, for: characteristic, type: .withResponse)
    }

    private func resetProvisioning() {
        provisioningFrames.removeAll()
        currentProvisioningFrame = nil
        provisioningPayloadSize = 0
        provisioningState = .idle
    }
}

extension NearbyDeviceScanner: @preconcurrency CBCentralManagerDelegate {
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        startScanningIfAvailable()
    }

    func centralManager(
        _ central: CBCentralManager,
        didDiscover peripheral: CBPeripheral,
        advertisementData: [String: Any],
        rssi RSSI: NSNumber
    ) {
        guard scanningRequested, case .scanning = state else { return }
        recordDiscovery(
            peripheral: peripheral,
            advertisementData: advertisementData,
            rssi: RSSI.intValue
        )
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        guard
            peripheral.identifier == selectedPeripheral?.identifier,
            let candidate = candidates[peripheral.identifier]
        else { return }

        state = .reading(candidate)
        peripheral.delegate = self
        peripheral.discoverServices([BLEContract.service])
    }

    func centralManager(
        _ central: CBCentralManager,
        didFailToConnect peripheral: CBPeripheral,
        error: (any Error)?
    ) {
        guard peripheral.identifier == selectedPeripheral?.identifier else { return }
        fail(error?.localizedDescription ?? "Could not connect to the nearby device.")
    }

    func centralManager(
        _ central: CBCentralManager,
        didDisconnectPeripheral peripheral: CBPeripheral,
        error: (any Error)?
    ) {
        guard peripheral.identifier == selectedPeripheral?.identifier else { return }
        fail(error?.localizedDescription ?? "The nearby device disconnected before its identity was read.")
    }
}

extension NearbyDeviceScanner: @preconcurrency CBPeripheralDelegate {
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: (any Error)?) {
        if let error {
            fail(error.localizedDescription)
            return
        }
        guard let service = peripheral.services?.first(where: { $0.uuid == BLEContract.service }) else {
            fail("The device does not expose the Extrittio contact service.")
            return
        }
        peripheral.discoverCharacteristics(BLEContract.allCharacteristics, for: service)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didDiscoverCharacteristicsFor service: CBService,
        error: (any Error)?
    ) {
        if let error {
            fail(error.localizedDescription)
            return
        }

        let identityCharacteristics = service.characteristics?.filter {
            BLEContract.identityCharacteristics.contains($0.uuid)
        } ?? []
        guard identityCharacteristics.count == BLEContract.identityCharacteristics.count else {
            fail("The device contact service is missing identity fields.")
            return
        }
        guard
            let messageCharacteristic = service.characteristics?.first(where: {
                $0.uuid == BLEContract.message && $0.properties.contains(.write)
            })
        else {
            fail("The device contact service does not support message writes.")
            return
        }
        self.messageCharacteristic = messageCharacteristic

        characteristicValues.removeAll()
        pendingCharacteristicUUIDs = Set(BLEContract.identityCharacteristics)
        for characteristic in identityCharacteristics {
            peripheral.readValue(for: characteristic)
        }
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didUpdateValueFor characteristic: CBCharacteristic,
        error: (any Error)?
    ) {
        if let error {
            fail(error.localizedDescription)
            return
        }
        guard
            pendingCharacteristicUUIDs.contains(characteristic.uuid),
            let data = characteristic.value,
            let value = String(data: data, encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines),
            !value.isEmpty
        else {
            fail("The device returned an unreadable identity field.")
            return
        }

        characteristicValues[characteristic.uuid] = value
        pendingCharacteristicUUIDs.remove(characteristic.uuid)
        guard pendingCharacteristicUUIDs.isEmpty else { return }
        guard let candidate = candidates[peripheral.identifier] else {
            fail("The nearby device could not be identified.")
            return
        }
        completeIdentityRead(candidate: candidate)
    }

    func peripheral(
        _ peripheral: CBPeripheral,
        didWriteValueFor characteristic: CBCharacteristic,
        error: (any Error)?
    ) {
        guard peripheral.identifier == selectedPeripheral?.identifier,
              characteristic.uuid == BLEContract.message
        else { return }

        if let frame = currentProvisioningFrame {
            if let error {
                logger.error("BLE provisioning write failed: \(error.localizedDescription, privacy: .public)")
                provisioningFrames.removeAll()
                currentProvisioningFrame = nil
                provisioningState = .failed("The device did not accept the provisioning data. Try again while keeping it nearby.")
            } else if let selectedPeripheral, let messageCharacteristic {
                provisioningState = .transferring(
                    completedBytes: frame.completedPayloadBytes,
                    totalBytes: provisioningPayloadSize
                )
                writeNextProvisioningFrame(to: selectedPeripheral, characteristic: messageCharacteristic)
            }
            return
        }

        guard case .writing(let request) = writeState else { return }

        if let error {
            logger.error("BLE contact write failed: \(error.localizedDescription, privacy: .public)")
            writeState = .failed(request, writeFailureMessage(for: request))
        } else {
            writeState = .succeeded(request)
        }
    }

    private func writeFailureMessage(for request: NearbyDeviceWriteRequest) -> String {
        switch request {
        case .pairingCommit:
            "The device could not confirm the connection. It may be busy or outside its normal operating state."
        case .command:
            "The device did not accept the command. Commands are unavailable during alerts or firmware updates."
        }
    }
}

private enum BLEContract {
    static let service = CBUUID(string: "9D8D0001-1A7C-4A21-9F25-E6B17EB89C11")
    static let deviceId = CBUUID(string: "9D8D0002-1A7C-4A21-9F25-E6B17EB89C11")
    static let model = CBUUID(string: "9D8D0003-1A7C-4A21-9F25-E6B17EB89C11")
    static let firmware = CBUUID(string: "9D8D0004-1A7C-4A21-9F25-E6B17EB89C11")
    static let transport = CBUUID(string: "9D8D0005-1A7C-4A21-9F25-E6B17EB89C11")
    static let message = CBUUID(string: "9D8D0006-1A7C-4A21-9F25-E6B17EB89C11")
    static let identityCharacteristics = [deviceId, model, firmware, transport]
    static let allCharacteristics = identityCharacteristics + [message]
}
