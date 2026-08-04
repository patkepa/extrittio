import Foundation
import SwiftData

@Model
final class CachedFirmwareUpdate {
    @Attribute(.unique) var id: Int
    var deviceTypeId: Int
    var deviceTypeName: String?
    var version: String
    var url: String
    var sha256: String?
    var descriptionText: String?
    var createdAt: String
    var hasBlob: Bool?
    var fileSize: Int?
    var filename: String?
    var source: String?
    var cachedAt: Date

    init(id: Int, deviceTypeId: Int, deviceTypeName: String?, version: String,
         url: String, sha256: String?, descriptionText: String?, createdAt: String,
         hasBlob: Bool?, fileSize: Int?, filename: String?, source: String?,
         cachedAt: Date = Date()) {
        self.id = id; self.deviceTypeId = deviceTypeId; self.deviceTypeName = deviceTypeName
        self.version = version; self.url = url; self.sha256 = sha256
        self.descriptionText = descriptionText; self.createdAt = createdAt
        self.hasBlob = hasBlob; self.fileSize = fileSize; self.filename = filename
        self.source = source; self.cachedAt = cachedAt
    }

    func toDomain() -> FirmwareUpdate {
        FirmwareUpdate(id: id, deviceTypeId: deviceTypeId, deviceTypeName: deviceTypeName,
                       version: version, url: url, sha256: sha256, description: descriptionText,
                       createdAt: createdAt, hasBlob: hasBlob, fileSize: fileSize,
                       filename: filename, source: source)
    }

    static func from(_ fw: FirmwareUpdate) -> CachedFirmwareUpdate {
        CachedFirmwareUpdate(id: fw.id, deviceTypeId: fw.deviceTypeId,
                             deviceTypeName: fw.deviceTypeName, version: fw.version,
                             url: fw.url, sha256: fw.sha256, descriptionText: fw.description,
                             createdAt: fw.createdAt, hasBlob: fw.hasBlob, fileSize: fw.fileSize,
                             filename: fw.filename, source: fw.source)
    }
}

@Model
final class CachedOtaDeployment {
    @Attribute(.unique) var id: Int
    var deviceId: String
    var firmwareUpdateId: Int
    var firmwareVersion: String?
    var status: String
    var errorMessage: String?
    var initiatedAt: String
    var completedAt: String?
    var cachedAt: Date

    init(id: Int, deviceId: String, firmwareUpdateId: Int, firmwareVersion: String?,
         status: String, errorMessage: String?, initiatedAt: String,
         completedAt: String?, cachedAt: Date = Date()) {
        self.id = id; self.deviceId = deviceId; self.firmwareUpdateId = firmwareUpdateId
        self.firmwareVersion = firmwareVersion; self.status = status
        self.errorMessage = errorMessage; self.initiatedAt = initiatedAt
        self.completedAt = completedAt; self.cachedAt = cachedAt
    }

    func toDomain() -> OtaDeployment {
        OtaDeployment(id: id, deviceId: deviceId, firmwareUpdateId: firmwareUpdateId,
                      firmwareVersion: firmwareVersion, status: status,
                      errorMessage: errorMessage, initiatedAt: initiatedAt, completedAt: completedAt)
    }

    static func from(_ dep: OtaDeployment) -> CachedOtaDeployment {
        CachedOtaDeployment(id: dep.id, deviceId: dep.deviceId,
                            firmwareUpdateId: dep.firmwareUpdateId,
                            firmwareVersion: dep.firmwareVersion, status: dep.status,
                            errorMessage: dep.errorMessage, initiatedAt: dep.initiatedAt,
                            completedAt: dep.completedAt)
    }
}
