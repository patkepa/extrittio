import Foundation
import os

@Observable
@MainActor
final class AdminSettingsViewModel {
    var usersState: ViewState<[User]> = .loading
    var rolesState: ViewState<[Role]> = .loading
    var permissionsState: ViewState<[PermissionResponse]> = .loading
    var apiKeysState: ViewState<[ApiKey]> = .loading
    var deviceTypesState: ViewState<[DeviceType]> = .loading
    var fleetsState: ViewState<[Fleet]> = .loading
    var caCertificateState: ViewState<CaCertificateResponse> = .loading
    var certificateDevicesState: ViewState<[Device]> = .loading
    var certificateStatuses: [String: DeviceCertificateStatusResponse] = [:]
    var lastCreatedApiKey: CreateApiKeyResponse?
    var certificateBundle: DeviceCertificateResponse?
    var errorMessage: String?

    private let userRepository: any UserRepository
    private let roleRepository: any RoleRepository
    private let apiKeyRepository: any ApiKeyRepository
    private let certificateRepository: any CertificateRepository
    private let deviceRepository: any DeviceRepository
    private let fleetRepository: any FleetRepository
    private let deviceTypeRepository: any DeviceTypeRepository
    private let logger = Logger(subsystem: "com.extrittio", category: "AdminSettings")

    init(
        userRepository: any UserRepository,
        roleRepository: any RoleRepository,
        apiKeyRepository: any ApiKeyRepository,
        certificateRepository: any CertificateRepository,
        deviceRepository: any DeviceRepository,
        fleetRepository: any FleetRepository,
        deviceTypeRepository: any DeviceTypeRepository
    ) {
        self.userRepository = userRepository
        self.roleRepository = roleRepository
        self.apiKeyRepository = apiKeyRepository
        self.certificateRepository = certificateRepository
        self.deviceRepository = deviceRepository
        self.fleetRepository = fleetRepository
        self.deviceTypeRepository = deviceTypeRepository
    }

    func loadUsers() async {
        usersState = .loading
        do {
            async let users = userRepository.getUsers()
            async let roles = roleRepository.getRoles()
            let loadedUsers = try await users
            let loadedRoles = try await roles
            usersState = loadedUsers.isEmpty ? .empty : .loaded(loadedUsers)
            rolesState = loadedRoles.isEmpty ? .empty : .loaded(loadedRoles)
        } catch {
            usersState = .error(error)
            logger.error("Users load failed: \(error)")
        }
    }

    func createUser(username: String, password: String, roleIds: [Int]) async -> Bool {
        do {
            _ = try await userRepository.createUser(AdminCreateUserRequest(username: username, password: password, roleIds: roleIds.isEmpty ? nil : roleIds))
            await loadUsers()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func deleteUser(id: Int) async -> Bool {
        do {
            try await userRepository.deleteUser(id: id)
            await loadUsers()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func setUserRoles(id: Int, roleIds: [Int]) async -> Bool {
        do {
            _ = try await userRepository.setUserRoles(id: id, roleIds: roleIds)
            await loadUsers()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func changePassword(id: Int, password: String) async -> Bool {
        do {
            try await userRepository.changePassword(id: id, password: password)
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func loadRoles() async {
        rolesState = .loading
        permissionsState = .loading
        do {
            async let roles = roleRepository.getRoles()
            async let permissions = roleRepository.getPermissions()
            let loadedRoles = try await roles
            let loadedPermissions = try await permissions
            rolesState = loadedRoles.isEmpty ? .empty : .loaded(loadedRoles)
            permissionsState = loadedPermissions.isEmpty ? .empty : .loaded(loadedPermissions)
        } catch {
            rolesState = .error(error)
            permissionsState = .error(error)
            logger.error("Roles load failed: \(error)")
        }
    }

    func createRole(name: String, description: String?, permissions: [String]) async -> Bool {
        do {
            _ = try await roleRepository.createRole(CreateRoleRequest(name: name, description: description, permissions: permissions))
            await loadRoles()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func updateRole(id: Int, description: String?, permissions: [String]) async -> Bool {
        do {
            _ = try await roleRepository.updateRole(id: id, UpdateRoleRequest(name: nil, description: description, permissions: permissions))
            await loadRoles()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func deleteRole(id: Int) async -> Bool {
        do {
            try await roleRepository.deleteRole(id: id)
            await loadRoles()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func loadApiKeys() async {
        apiKeysState = .loading
        do {
            let keys = try await apiKeyRepository.getApiKeys()
            apiKeysState = keys.isEmpty ? .empty : .loaded(keys)
        } catch {
            apiKeysState = .error(error)
            logger.error("API keys load failed: \(error)")
        }
    }

    func createApiKey(name: String, deviceTypeId: Int?) async -> Bool {
        do {
            lastCreatedApiKey = try await apiKeyRepository.createApiKey(CreateApiKeyRequest(name: name, deviceTypeId: deviceTypeId))
            await loadApiKeys()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func deleteApiKey(id: Int) async -> Bool {
        do {
            try await apiKeyRepository.deleteApiKey(id: id)
            await loadApiKeys()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func loadDeviceTypes() async {
        deviceTypesState = .loading
        do {
            let types = try await deviceTypeRepository.getDeviceTypes()
            deviceTypesState = types.isEmpty ? .empty : .loaded(types)
        } catch {
            deviceTypesState = .error(error)
            logger.error("Device types load failed: \(error)")
        }
    }

    func createDeviceType(name: String, icon: String?, colorHex: String?) async -> Bool {
        do {
            _ = try await deviceTypeRepository.createDeviceType(CreateDeviceTypeRequest(name: name, icon: icon, colorHex: colorHex))
            await loadDeviceTypes()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func updateDeviceType(id: Int, name: String, icon: String?, colorHex: String?) async -> Bool {
        do {
            _ = try await deviceTypeRepository.updateDeviceType(id: id, UpdateDeviceTypeRequest(name: name, icon: icon, colorHex: colorHex))
            await loadDeviceTypes()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func deleteDeviceType(id: Int) async -> Bool {
        do {
            try await deviceTypeRepository.deleteDeviceType(id: id)
            await loadDeviceTypes()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func loadFleets() async {
        fleetsState = .loading
        do {
            let fleets = try await fleetRepository.getFleets()
            fleetsState = fleets.isEmpty ? .empty : .loaded(fleets)
        } catch {
            fleetsState = .error(error)
            logger.error("Fleets load failed: \(error)")
        }
    }

    func createFleet(name: String) async -> Bool {
        do {
            _ = try await fleetRepository.createFleet(CreateFleetRequest(name: name))
            await loadFleets()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func updateFleet(id: Int, name: String) async -> Bool {
        do {
            _ = try await fleetRepository.updateFleet(id: id, UpdateFleetRequest(name: name))
            await loadFleets()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func deleteFleet(id: Int) async -> Bool {
        do {
            try await fleetRepository.deleteFleet(id: id)
            await loadFleets()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }

    func loadCertificates() async {
        caCertificateState = .loading
        certificateDevicesState = .loading
        certificateStatuses = [:]

        do {
            let ca = try await certificateRepository.getCaCertificate()
            caCertificateState = .loaded(ca)
        } catch {
            if let apiError = error as? APIError, apiError == .notFound {
                caCertificateState = .empty
            } else {
                caCertificateState = .error(error)
            }
        }

        do {
            let devices = try await deviceRepository.getDevices(status: nil, search: nil, fleetId: nil, limit: 1000, offset: 0).data
            certificateDevicesState = devices.isEmpty ? .empty : .loaded(devices)
            var statuses: [String: DeviceCertificateStatusResponse] = [:]
            for device in devices {
                if let status = try? await certificateRepository.getDeviceCertificateStatus(deviceId: device.id) {
                    statuses[device.id] = status
                }
            }
            certificateStatuses = statuses
        } catch {
            certificateDevicesState = .error(error)
            logger.error("Certificate devices load failed: \(error)")
        }
    }

    func loadDeviceCertificate(deviceId: String, regenerate: Bool) async -> Bool {
        do {
            certificateBundle = regenerate
                ? try await certificateRepository.regenerateDeviceCertificate(deviceId: deviceId)
                : try await certificateRepository.getDeviceCertificate(deviceId: deviceId)
            await loadCertificates()
            return true
        } catch {
            errorMessage = error.localizedDescription
            return false
        }
    }
}
