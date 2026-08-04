import SwiftUI

struct UsersSettingsView: View {
    let viewModel: AdminSettingsViewModel
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @State private var showingCreate = false
    @State private var editingRolesForUser: User?
    @State private var changingPasswordForUser: User?
    @State private var deletingUserId: Int?

    private var canManage: Bool { authViewModel.currentUser?.hasPermission(.usersManage) == true }

    var body: some View {
        List {
            if !connectionMonitor.isOnline, canManage {
                OfflineActionHint(message: "User changes are unavailable while offline.")
                    .adminListRow()
            }

            switch viewModel.usersState {
            case .loading:
                AdminSkeletonRows()
            case .empty:
                EmptyStateView(
                    icon: "person.2",
                    title: "No Users",
                    message: "No users were returned by the server.",
                    actionTitle: canManage ? "Add User" : nil,
                    actionSystemImage: "plus",
                    isActionDisabled: !connectionMonitor.isOnline
                ) {
                    showingCreate = true
                }
                .adminListRow()
            case .error(let error):
                ErrorBanner(message: error.localizedDescription) {
                    Task { await viewModel.loadUsers() }
                }
                .adminListRow()
            case .loaded(let users), .cached(let users, _), .loadingMore(let users):
                ForEach(users) { user in
                    userRow(user)
                        .adminListRow()
                }
            }
        }
        .listStyle(.plain)
        .navigationTitle("Users")
        .toolbar {
            if canManage {
                Button {
                    showingCreate = true
                } label: {
                    Label("Add User", systemImage: "plus")
                }
                .disabled(!connectionMonitor.isOnline)
            }
        }
        .refreshable {
            await viewModel.loadUsers()
            if canManage {
                await viewModel.loadRoles()
            }
        }
        .task {
            await viewModel.loadUsers()
            if canManage {
                await viewModel.loadRoles()
            }
        }
        .sheet(isPresented: $showingCreate) {
            UserCreateSheet(viewModel: viewModel)
        }
        .sheet(item: $editingRolesForUser) { user in
            UserRolesSheet(user: user, roles: viewModel.rolesState.data ?? []) { roleIds in
                let success = await viewModel.setUserRoles(id: user.id, roleIds: roleIds)
                toastManager.show(success ? .success("Roles updated") : .error(viewModel.errorMessage ?? "Failed to update roles"))
                return success
            }
        }
        .sheet(item: $changingPasswordForUser) { user in
            PasswordChangeSheet(user: user) { password in
                let success = await viewModel.changePassword(id: user.id, password: password)
                toastManager.show(success ? .success("Password updated") : .error(viewModel.errorMessage ?? "Failed to update password"))
                return success
            }
        }
    }

    private func userRow(_ user: User) -> some View {
        AdminRowShell(isWorking: deletingUserId == user.id) {
            VStack(alignment: .leading, spacing: 8) {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(user.username)
                        .font(.headline)
                    Text(user.roles?.map(\.name).joined(separator: ", ") ?? user.role)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                if deletingUserId == user.id {
                    ProgressView()
                        .controlSize(.small)
                } else if canManage {
                    Menu {
                        Button("Edit Roles") { editingRolesForUser = user }
                            .disabled(!connectionMonitor.isOnline)
                        Button("Change Password") { changingPasswordForUser = user }
                            .disabled(!connectionMonitor.isOnline)
                        Button("Delete", role: .destructive) {
                            deletingUserId = user.id
                            Task {
                                let success = await viewModel.deleteUser(id: user.id)
                                deletingUserId = nil
                                toastManager.show(success ? .success("User deleted") : .error(viewModel.errorMessage ?? "Failed to delete user"))
                            }
                        }
                        .disabled(!connectionMonitor.isOnline || deletingUserId != nil)
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
            if let permissions = user.permissions, !permissions.isEmpty {
                Text("\(permissions.count) permissions")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
        }
        }
    }
}

private struct UserCreateSheet: View {
    let viewModel: AdminSettingsViewModel
    @Environment(\.dismiss) private var dismiss
    @Environment(ToastManager.self) private var toastManager
    @State private var username = ""
    @State private var password = ""
    @State private var roleIds = Set<Int>()
    @State private var isSaving = false

    var body: some View {
        NavigationStack {
            Form {
                TextField("Username", text: $username)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
                SecureField("Password", text: $password)
                RolePicker(roles: viewModel.rolesState.data ?? [], selectedRoleIds: $roleIds)
            }
            .navigationTitle("Add User")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task {
                            isSaving = true
                            let success = await viewModel.createUser(username: username, password: password, roleIds: Array(roleIds))
                            isSaving = false
                            toastManager.show(success ? .success("User created") : .error(viewModel.errorMessage ?? "Failed to create user"))
                            if success { dismiss() }
                        }
                    }
                    .disabled(username.isEmpty || password.isEmpty || isSaving)
                }
            }
        }
    }
}

private struct UserRolesSheet: View {
    let user: User
    let roles: [Role]
    let onSave: ([Int]) async -> Bool
    @Environment(\.dismiss) private var dismiss
    @State private var roleIds: Set<Int>
    @State private var isSaving = false

    init(user: User, roles: [Role], onSave: @escaping ([Int]) async -> Bool) {
        self.user = user
        self.roles = roles
        self.onSave = onSave
        self._roleIds = State(initialValue: Set(user.roles?.map(\.id) ?? []))
    }

    var body: some View {
        NavigationStack {
            Form {
                RolePicker(roles: roles, selectedRoleIds: $roleIds)
            }
            .navigationTitle("Roles: \(user.username)")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        Task {
                            isSaving = true
                            let success = await onSave(Array(roleIds))
                            isSaving = false
                            if success { dismiss() }
                        }
                    }
                    .disabled(roleIds.isEmpty || isSaving)
                }
            }
        }
    }
}

private struct PasswordChangeSheet: View {
    let user: User
    let onSave: (String) async -> Bool
    @Environment(\.dismiss) private var dismiss
    @State private var password = ""
    @State private var isSaving = false

    var body: some View {
        NavigationStack {
            Form {
                SecureField("New Password", text: $password)
            }
            .navigationTitle("Password: \(user.username)")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        Task {
                            isSaving = true
                            let success = await onSave(password)
                            isSaving = false
                            if success { dismiss() }
                        }
                    }
                    .disabled(password.isEmpty || isSaving)
                }
            }
        }
    }
}

private struct RolePicker: View {
    let roles: [Role]
    @Binding var selectedRoleIds: Set<Int>

    var body: some View {
        Section("Roles") {
            if roles.isEmpty {
                Text("No roles available")
                    .foregroundStyle(.secondary)
            } else {
                ForEach(roles) { role in
                    Toggle(role.name, isOn: Binding(
                        get: { selectedRoleIds.contains(role.id) },
                        set: { isOn in
                            if isOn { selectedRoleIds.insert(role.id) } else { selectedRoleIds.remove(role.id) }
                        }
                    ))
                }
            }
        }
    }
}

struct RolesSettingsView: View {
    let viewModel: AdminSettingsViewModel
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @State private var showingCreate = false
    @State private var editingRole: Role?
    @State private var deletingRoleId: Int?

    private var canManage: Bool { authViewModel.currentUser?.hasPermission(.rolesManage) == true }

    var body: some View {
        List {
            if !connectionMonitor.isOnline, canManage {
                OfflineActionHint(message: "Role changes are unavailable while offline.")
                    .adminListRow()
            }

            switch viewModel.rolesState {
            case .loading:
                AdminSkeletonRows()
            case .empty:
                EmptyStateView(
                    icon: "shield",
                    title: "No Roles",
                    message: "No roles were returned by the server.",
                    actionTitle: canManage ? "Add Role" : nil,
                    actionSystemImage: "plus",
                    isActionDisabled: !connectionMonitor.isOnline
                ) {
                    showingCreate = true
                }
                .adminListRow()
            case .error(let error):
                ErrorBanner(message: error.localizedDescription) {
                    Task { await viewModel.loadRoles() }
                }
                .adminListRow()
            case .loaded(let roles), .cached(let roles, _), .loadingMore(let roles):
                ForEach(roles) { role in
                    roleRow(role)
                        .adminListRow()
                }
            }
        }
        .listStyle(.plain)
        .navigationTitle("Roles")
        .toolbar {
            if canManage {
                Button {
                    showingCreate = true
                } label: {
                    Label("Add Role", systemImage: "plus")
                }
                .disabled(!connectionMonitor.isOnline)
            }
        }
        .refreshable {
            await viewModel.loadRoles()
        }
        .task {
            await viewModel.loadRoles()
        }
        .sheet(isPresented: $showingCreate) {
            RoleFormSheet(
                title: "Add Role",
                role: nil,
                permissions: viewModel.permissionsState.data ?? []
            ) { name, description, permissions in
                let success = await viewModel.createRole(name: name, description: description, permissions: permissions)
                toastManager.show(success ? .success("Role created") : .error(viewModel.errorMessage ?? "Failed to create role"))
                return success
            }
        }
        .sheet(item: $editingRole) { role in
            RoleFormSheet(
                title: "Edit Role",
                role: role,
                permissions: viewModel.permissionsState.data ?? []
            ) { _, description, permissions in
                let success = await viewModel.updateRole(id: role.id, description: description, permissions: permissions)
                toastManager.show(success ? .success("Role updated") : .error(viewModel.errorMessage ?? "Failed to update role"))
                return success
            }
        }
    }

    private func roleRow(_ role: Role) -> some View {
        AdminRowShell(isWorking: deletingRoleId == role.id) {
            VStack(alignment: .leading, spacing: 6) {
            HStack {
                VStack(alignment: .leading) {
                    HStack {
                        Text(role.name)
                            .font(.headline)
                        if role.isSystem {
                            AdminMetadataBadge(text: "Built-in", tint: .blue)
                        }
                    }
                    Text("\(role.permissions.count) permissions - \(role.userCount) users")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                if deletingRoleId == role.id {
                    ProgressView()
                        .controlSize(.small)
                } else if canManage {
                    Menu {
                        Button("Edit") { editingRole = role }
                            .disabled(role.isSystem || !connectionMonitor.isOnline)
                        Button("Delete", role: .destructive) {
                            deletingRoleId = role.id
                            Task {
                                let success = await viewModel.deleteRole(id: role.id)
                                deletingRoleId = nil
                                toastManager.show(success ? .success("Role deleted") : .error(viewModel.errorMessage ?? "Failed to delete role"))
                            }
                        }
                        .disabled(role.isSystem || role.userCount > 0 || !connectionMonitor.isOnline || deletingRoleId != nil)
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
            if let description = role.description, !description.isEmpty {
                Text(description)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        }
    }
}

private struct RoleFormSheet: View {
    let title: String
    let role: Role?
    let permissions: [PermissionResponse]
    let onSave: (String, String?, [String]) async -> Bool
    @Environment(\.dismiss) private var dismiss
    @State private var name: String
    @State private var description: String
    @State private var selectedPermissions: Set<String>
    @State private var isSaving = false

    init(title: String, role: Role?, permissions: [PermissionResponse], onSave: @escaping (String, String?, [String]) async -> Bool) {
        self.title = title
        self.role = role
        self.permissions = permissions
        self.onSave = onSave
        self._name = State(initialValue: role?.name ?? "")
        self._description = State(initialValue: role?.description ?? "")
        self._selectedPermissions = State(initialValue: Set(role?.permissions ?? []))
    }

    var body: some View {
        NavigationStack {
            Form {
                TextField("Name", text: $name)
                    .disabled(role != nil)
                    .textInputAutocapitalization(.never)
                TextField("Description", text: $description)
                Section("Permissions") {
                    ForEach(permissions) { permission in
                        Toggle(permission.key, isOn: Binding(
                            get: { selectedPermissions.contains(permission.key) },
                            set: { isOn in
                                if isOn { selectedPermissions.insert(permission.key) } else { selectedPermissions.remove(permission.key) }
                            }
                        ))
                    }
                }
            }
            .navigationTitle(title)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        Task {
                            isSaving = true
                            let success = await onSave(name, description.isEmpty ? nil : description, Array(selectedPermissions).sorted())
                            isSaving = false
                            if success { dismiss() }
                        }
                    }
                    .disabled(name.isEmpty || selectedPermissions.isEmpty || isSaving)
                }
            }
        }
    }
}

struct ApiKeysSettingsView: View {
    let viewModel: AdminSettingsViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @State private var showingCreate = false
    @State private var generatedKey: CreateApiKeyResponse?
    @State private var deletingApiKeyId: Int?

    var body: some View {
        List {
            if !connectionMonitor.isOnline {
                OfflineActionHint(message: "API key changes are unavailable while offline.")
                    .adminListRow()
            }

            switch viewModel.apiKeysState {
            case .loading:
                AdminSkeletonRows()
            case .empty:
                EmptyStateView(
                    icon: "key",
                    title: "No API Keys",
                    message: "Create API keys for CI or automated firmware registration.",
                    actionTitle: "Create Key",
                    actionSystemImage: "plus",
                    isActionDisabled: !connectionMonitor.isOnline
                ) {
                    showingCreate = true
                }
                .adminListRow()
            case .error(let error):
                ErrorBanner(message: error.localizedDescription) {
                    Task { await viewModel.loadApiKeys() }
                }
                .adminListRow()
            case .loaded(let keys), .cached(let keys, _), .loadingMore(let keys):
                ForEach(keys) { key in
                    AdminRowShell(isWorking: deletingApiKeyId == key.id) {
                        VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text(key.name)
                                .font(.headline)
                            Spacer()
                            if deletingApiKeyId == key.id {
                                ProgressView()
                                    .controlSize(.small)
                            } else {
                                Button(role: .destructive) {
                                    deletingApiKeyId = key.id
                                    Task {
                                        let success = await viewModel.deleteApiKey(id: key.id)
                                        deletingApiKeyId = nil
                                        toastManager.show(success ? .success("API key revoked") : .error(viewModel.errorMessage ?? "Failed to revoke API key"))
                                    }
                                } label: {
                                    Image(systemName: "trash")
                                }
                                .disabled(!connectionMonitor.isOnline || deletingApiKeyId != nil)
                            }
                        }
                        Text("\(key.keyPrefix)...")
                            .font(.caption.monospaced())
                            .foregroundStyle(.secondary)
                        Text(key.deviceTypeName ?? "All device types")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    }
                    .adminListRow()
                }
            }
        }
        .listStyle(.plain)
        .navigationTitle("API Keys")
        .toolbar {
            Button {
                showingCreate = true
            } label: {
                Label("Create", systemImage: "plus")
            }
            .disabled(!connectionMonitor.isOnline)
        }
        .refreshable { await viewModel.loadApiKeys() }
        .task {
            await viewModel.loadApiKeys()
            await viewModel.loadDeviceTypes()
        }
        .sheet(isPresented: $showingCreate) {
            ApiKeyCreateSheet(viewModel: viewModel) { key in
                generatedKey = key
            }
        }
        .sheet(item: $generatedKey) { key in
            SecretDisplaySheet(title: "API Key Created", secret: key.key, detail: "This key is shown once.")
        }
    }
}

private struct ApiKeyCreateSheet: View {
    let viewModel: AdminSettingsViewModel
    let onCreated: (CreateApiKeyResponse) -> Void
    @Environment(\.dismiss) private var dismiss
    @Environment(ToastManager.self) private var toastManager
    @State private var name = ""
    @State private var deviceTypeId: Int?
    @State private var isSaving = false

    var body: some View {
        NavigationStack {
            Form {
                TextField("Name", text: $name)
                Picker("Scope", selection: $deviceTypeId) {
                    Text("All device types").tag(nil as Int?)
                    ForEach(viewModel.deviceTypesState.data ?? []) { type in
                        Text(type.name).tag(type.id as Int?)
                    }
                }
            }
            .navigationTitle("Create API Key")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        Task {
                            isSaving = true
                            let success = await viewModel.createApiKey(name: name, deviceTypeId: deviceTypeId)
                            isSaving = false
                            if success, let key = viewModel.lastCreatedApiKey {
                                toastManager.show(.success("API key created"))
                                dismiss()
                                onCreated(key)
                            } else {
                                toastManager.show(.error(viewModel.errorMessage ?? "Failed to create API key"))
                            }
                        }
                    }
                    .disabled(name.isEmpty || isSaving)
                }
            }
        }
    }
}

struct CertificatesSettingsView: View {
    let viewModel: AdminSettingsViewModel
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @State private var bundle: DeviceCertificateResponse?
    @State private var loadingBundleDeviceId: String?

    private var canManageCertificates: Bool {
        authViewModel.currentUser?.hasPermission(.devicesManage) == true
    }

    var body: some View {
        List {
            if !connectionMonitor.isOnline, canManageCertificates {
                OfflineActionHint(message: "Certificate generation is unavailable while offline.")
                    .adminListRow()
            }

            Section("CA Certificate") {
                switch viewModel.caCertificateState {
                case .loading:
                    AdminSkeletonRows(count: 1)
                case .empty:
                    Text("CA certificate is not initialized.")
                        .foregroundStyle(.secondary)
                        .adminListRow()
                case .error(let error):
                    ErrorBanner(message: error.localizedDescription) {
                        Task { await viewModel.loadCertificates() }
                    }
                    .adminListRow()
                case .loaded(let cert), .cached(let cert, _), .loadingMore(let cert):
                    AdminRowShell {
                        VStack(alignment: .leading, spacing: 6) {
                            Text(cert.fingerprint)
                                .font(.caption.monospaced())
                            Text("Created \(cert.createdAt)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Button("Copy CA PEM") {
                                UIPasteboard.general.string = cert.certificatePem
                                toastManager.show(.copied)
                            }
                        }
                    }
                    .adminListRow()
                }
            }

            Section("Device Certificates") {
                switch viewModel.certificateDevicesState {
                case .loading:
                    AdminSkeletonRows()
                case .empty:
                    EmptyStateView(icon: "lock", title: "No Devices", message: "No devices are available for certificate generation.")
                        .adminListRow()
                case .error(let error):
                    ErrorBanner(message: error.localizedDescription) {
                        Task { await viewModel.loadCertificates() }
                    }
                    .adminListRow()
                case .loaded(let devices), .cached(let devices, _), .loadingMore(let devices):
                    ForEach(devices) { device in
                        certificateRow(device)
                            .adminListRow()
                    }
                }
            }
        }
        .listStyle(.plain)
        .navigationTitle("Certificates")
        .refreshable { await viewModel.loadCertificates() }
        .task { await viewModel.loadCertificates() }
        .sheet(item: $bundle) { cert in
            CertificateBundleSheet(bundle: cert)
        }
    }

    private func certificateRow(_ device: Device) -> some View {
        AdminRowShell(isWorking: loadingBundleDeviceId == device.id) {
            VStack(alignment: .leading, spacing: 6) {
            HStack {
                VStack(alignment: .leading) {
                    Text(device.name)
                        .font(.headline)
                    Text(device.id)
                        .font(.caption.monospaced())
                        .foregroundStyle(.secondary)
                }
                Spacer()
                if loadingBundleDeviceId == device.id {
                    ProgressView()
                        .controlSize(.small)
                } else if canManageCertificates {
                    Menu {
                        Button("Get Bundle") {
                            Task { await loadBundle(deviceId: device.id, regenerate: false) }
                        }
                        .disabled(!connectionMonitor.isOnline || loadingBundleDeviceId != nil)
                        Button("Regenerate") {
                            Task { await loadBundle(deviceId: device.id, regenerate: true) }
                        }
                        .disabled(!connectionMonitor.isOnline || loadingBundleDeviceId != nil)
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
            if let status = viewModel.certificateStatuses[device.id] {
                Text("Expires \(status.expiresAt)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                Text("No certificate")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        }
    }

    private func loadBundle(deviceId: String, regenerate: Bool) async {
        loadingBundleDeviceId = deviceId
        defer { loadingBundleDeviceId = nil }
        let success = await viewModel.loadDeviceCertificate(deviceId: deviceId, regenerate: regenerate)
        if success, let cert = viewModel.certificateBundle {
            bundle = cert
        } else {
            toastManager.show(.error(viewModel.errorMessage ?? "Failed to load certificate"))
        }
    }
}

private struct CertificateBundleSheet: View {
    let bundle: DeviceCertificateResponse
    @Environment(\.dismiss) private var dismiss
    @Environment(ToastManager.self) private var toastManager

    var body: some View {
        NavigationStack {
            List {
                SecretCopyRow(title: "Certificate PEM", value: bundle.certificatePem)
                SecretCopyRow(title: "Private Key PEM", value: bundle.privateKeyPem)
                SecretCopyRow(title: "CA PEM", value: bundle.caPem)
                Text("Fingerprint: \(bundle.fingerprint)")
                    .font(.caption.monospaced())
                Text("Expires \(bundle.expiresAt)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .navigationTitle("Certificate Bundle")
            .toolbar {
                Button("Done") { dismiss() }
            }
        }
    }
}

struct DeviceTypesSettingsView: View {
    let viewModel: AdminSettingsViewModel
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @State private var showingCreate = false
    @State private var editingType: DeviceType?
    @State private var deletingTypeId: Int?

    private var canManage: Bool {
        authViewModel.currentUser?.hasPermission(.deviceTypesManage) == true
    }

    var body: some View {
        List {
            if !connectionMonitor.isOnline, canManage {
                OfflineActionHint(message: "Device type changes are unavailable while offline.")
                    .adminListRow()
            }

            switch viewModel.deviceTypesState {
            case .loading:
                AdminSkeletonRows()
            case .empty:
                EmptyStateView(
                    icon: "tag",
                    title: "No Device Types",
                    message: "Create a device type to categorize devices.",
                    actionTitle: canManage ? "Add Device Type" : nil,
                    actionSystemImage: "plus",
                    isActionDisabled: !connectionMonitor.isOnline
                ) {
                    showingCreate = true
                }
                .adminListRow()
            case .error(let error):
                ErrorBanner(message: error.localizedDescription) {
                    Task { await viewModel.loadDeviceTypes() }
                }
                .adminListRow()
            case .loaded(let types), .cached(let types, _), .loadingMore(let types):
                ForEach(types) { type in
                    AdminRowShell(isWorking: deletingTypeId == type.id) {
                        HStack(spacing: Spacing.md) {
                            DeviceMockupView(
                                deviceTypeName: type.name,
                                icon: type.icon,
                                colorHex: type.colorHex,
                                size: .compact
                            )
                            .frame(width: 54, height: 48)

                            VStack(alignment: .leading, spacing: Spacing.xs) {
                                Text(type.name)
                                    .font(.body.weight(.medium))
                                if let icon = type.icon, !icon.isEmpty {
                                    Text(icon)
                                        .font(.caption.monospaced())
                                        .foregroundStyle(.secondary)
                                        .lineLimit(1)
                                }
                            }
                            Spacer()
                            if deletingTypeId == type.id {
                                ProgressView()
                                    .controlSize(.small)
                            } else if canManage {
                                Menu {
                                    Button("Edit") { editingType = type }
                                        .disabled(!connectionMonitor.isOnline)
                                    Button("Delete", role: .destructive) {
                                        deletingTypeId = type.id
                                        Task {
                                            let success = await viewModel.deleteDeviceType(id: type.id)
                                            deletingTypeId = nil
                                            toastManager.show(success ? .success("Device type deleted") : .error(viewModel.errorMessage ?? "Failed to delete device type"))
                                        }
                                    }
                                    .disabled(!connectionMonitor.isOnline || deletingTypeId != nil)
                                } label: {
                                    Image(systemName: "ellipsis.circle")
                                    }
                            }
                        }
                    }
                    .adminListRow()
                }
            }
        }
        .listStyle(.plain)
        .navigationTitle("Device Types")
        .toolbar {
            if canManage {
                Button {
                    showingCreate = true
                } label: {
                    Label("Add", systemImage: "plus")
                }
                .disabled(!connectionMonitor.isOnline)
            }
        }
        .refreshable { await viewModel.loadDeviceTypes() }
        .task { await viewModel.loadDeviceTypes() }
        .sheet(isPresented: $showingCreate) {
            DeviceTypeFormSheet(type: nil) { name, icon, color in
                let success = await viewModel.createDeviceType(name: name, icon: icon, colorHex: color)
                toastManager.show(success ? .success("Device type created") : .error(viewModel.errorMessage ?? "Failed to create device type"))
                return success
            }
        }
        .sheet(item: $editingType) { type in
            DeviceTypeFormSheet(type: type) { name, icon, color in
                let success = await viewModel.updateDeviceType(id: type.id, name: name, icon: icon, colorHex: color)
                toastManager.show(success ? .success("Device type updated") : .error(viewModel.errorMessage ?? "Failed to update device type"))
                return success
            }
        }
    }
}

private struct DeviceTypeFormSheet: View {
    let type: DeviceType?
    let onSave: (String, String?, String?) async -> Bool
    @Environment(\.dismiss) private var dismiss
    @State private var name: String
    @State private var icon: String
    @State private var colorHex: String
    @State private var isSaving = false

    init(type: DeviceType?, onSave: @escaping (String, String?, String?) async -> Bool) {
        self.type = type
        self.onSave = onSave
        self._name = State(initialValue: type?.name ?? "")
        self._icon = State(initialValue: type?.icon ?? "cube")
        self._colorHex = State(initialValue: type?.colorHex ?? "#8ABBFF")
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    HStack {
                        Spacer()
                        DeviceMockupView(
                            deviceTypeName: name,
                            icon: icon.isEmpty ? nil : icon,
                            colorHex: colorHex.isEmpty ? nil : colorHex,
                            size: .large
                        )
                        Spacer()
                    }
                    .padding(.vertical, Spacing.sm)
                }
                TextField("Name", text: $name)
                TextField("Icon", text: $icon)
                    .textInputAutocapitalization(.never)
                TextField("Color Hex", text: $colorHex)
                    .textInputAutocapitalization(.never)
            }
            .navigationTitle(type == nil ? "Add Device Type" : "Edit Device Type")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        Task {
                            isSaving = true
                            let success = await onSave(name, icon.isEmpty ? nil : icon, colorHex.isEmpty ? nil : colorHex)
                            isSaving = false
                            if success { dismiss() }
                        }
                    }
                    .disabled(name.isEmpty || isSaving)
                }
            }
        }
    }
}

struct FleetsSettingsView: View {
    let viewModel: AdminSettingsViewModel
    @Environment(AuthViewModel.self) private var authViewModel
    @Environment(ToastManager.self) private var toastManager
    @Environment(ConnectionMonitor.self) private var connectionMonitor
    @State private var showingCreate = false
    @State private var editingFleet: Fleet?
    @State private var deletingFleetId: Int?

    private var canManage: Bool {
        authViewModel.currentUser?.hasPermission(.fleetsManage) == true
    }

    var body: some View {
        List {
            if !connectionMonitor.isOnline, canManage {
                OfflineActionHint(message: "Fleet changes are unavailable while offline.")
                    .adminListRow()
            }

            switch viewModel.fleetsState {
            case .loading:
                AdminSkeletonRows()
            case .empty:
                EmptyStateView(
                    icon: "folder",
                    title: "No Fleets",
                    message: "Create fleets to group devices.",
                    actionTitle: canManage ? "Add Fleet" : nil,
                    actionSystemImage: "plus",
                    isActionDisabled: !connectionMonitor.isOnline
                ) {
                    showingCreate = true
                }
                .adminListRow()
            case .error(let error):
                ErrorBanner(message: error.localizedDescription) {
                    Task { await viewModel.loadFleets() }
                }
                .adminListRow()
            case .loaded(let fleets), .cached(let fleets, _), .loadingMore(let fleets):
                ForEach(fleets) { fleet in
                    AdminRowShell(isWorking: deletingFleetId == fleet.id) {
                        HStack {
                            VStack(alignment: .leading) {
                                Text(fleet.name)
                                    .font(.body.weight(.medium))
                                Text("\(fleet.deviceCount ?? 0) devices")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            if deletingFleetId == fleet.id {
                                ProgressView()
                                    .controlSize(.small)
                            } else if canManage {
                                Menu {
                                    Button("Rename") { editingFleet = fleet }
                                        .disabled(!connectionMonitor.isOnline)
                                    Button("Delete", role: .destructive) {
                                        deletingFleetId = fleet.id
                                        Task {
                                            let success = await viewModel.deleteFleet(id: fleet.id)
                                            deletingFleetId = nil
                                            toastManager.show(success ? .success("Fleet deleted") : .error(viewModel.errorMessage ?? "Failed to delete fleet"))
                                        }
                                    }
                                    .disabled(!connectionMonitor.isOnline || deletingFleetId != nil)
                                } label: {
                                    Image(systemName: "ellipsis.circle")
                                    }
                            }
                        }
                    }
                    .adminListRow()
                }
            }
        }
        .listStyle(.plain)
        .navigationTitle("Fleets")
        .toolbar {
            if canManage {
                Button {
                    showingCreate = true
                } label: {
                    Label("Add", systemImage: "plus")
                }
                .disabled(!connectionMonitor.isOnline)
            }
        }
        .refreshable { await viewModel.loadFleets() }
        .task { await viewModel.loadFleets() }
        .sheet(isPresented: $showingCreate) {
            FleetFormSheet(fleet: nil) { name in
                let success = await viewModel.createFleet(name: name)
                toastManager.show(success ? .success("Fleet created") : .error(viewModel.errorMessage ?? "Failed to create fleet"))
                return success
            }
        }
        .sheet(item: $editingFleet) { fleet in
            FleetFormSheet(fleet: fleet) { name in
                let success = await viewModel.updateFleet(id: fleet.id, name: name)
                toastManager.show(success ? .success("Fleet updated") : .error(viewModel.errorMessage ?? "Failed to update fleet"))
                return success
            }
        }
    }
}

private struct FleetFormSheet: View {
    let fleet: Fleet?
    let onSave: (String) async -> Bool
    @Environment(\.dismiss) private var dismiss
    @State private var name: String
    @State private var isSaving = false

    init(fleet: Fleet?, onSave: @escaping (String) async -> Bool) {
        self.fleet = fleet
        self.onSave = onSave
        self._name = State(initialValue: fleet?.name ?? "")
    }

    var body: some View {
        NavigationStack {
            Form {
                TextField("Name", text: $name)
            }
            .navigationTitle(fleet == nil ? "Add Fleet" : "Rename Fleet")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        Task {
                            isSaving = true
                            let success = await onSave(name)
                            isSaving = false
                            if success { dismiss() }
                        }
                    }
                    .disabled(name.isEmpty || isSaving)
                }
            }
        }
    }
}

private struct SecretDisplaySheet: View {
    let title: String
    let secret: String
    let detail: String
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Text(detail)
                    .foregroundStyle(.secondary)
                SecretCopyRow(title: "Secret", value: secret)
            }
            .navigationTitle(title)
            .toolbar {
                Button("Done") { dismiss() }
            }
        }
    }
}

private struct SecretCopyRow: View {
    let title: String
    let value: String
    @Environment(ToastManager.self) private var toastManager

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.headline)
            Text(value)
                .font(.caption.monospaced())
                .lineLimit(5)
                .textSelection(.enabled)
            Button("Copy") {
                UIPasteboard.general.string = value
                toastManager.show(.copied)
            }
        }
        .padding(.vertical, 4)
    }
}
