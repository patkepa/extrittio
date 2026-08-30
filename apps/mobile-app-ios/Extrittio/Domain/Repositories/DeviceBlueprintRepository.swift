import Foundation

protocol DeviceBlueprintRepository: Sendable {
    func getBlueprints() async throws -> [DeviceBlueprint]
    func getLatestRevision(blueprintId: String) async throws -> DeviceBlueprintRevision
}
