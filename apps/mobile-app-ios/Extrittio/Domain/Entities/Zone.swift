import Foundation

struct Zone: Codable, Sendable, Identifiable {
    let id: String
    let name: String
    let description: String
    let geometryType: String
    let geometryJson: ZoneGeometry
    let color: String
    let createdAt: String
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id, name, description, color
        case geometryType = "geometry_type"
        case geometryJson = "geometry_json"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }

    var isCircle: Bool { geometryType == "circle" }
    var isPolygon: Bool { geometryType == "polygon" }
}

enum ZoneGeometry: Codable, Sendable {
    case circle(centerLat: Double, centerLon: Double, radiusMeters: Double)
    case polygon(points: [[Double]])

    private struct CircleJSON: Codable {
        let center_lat: Double
        let center_lon: Double
        let radius_meters: Double
    }

    private struct PolygonJSON: Codable {
        let points: [[Double]]
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let circle = try? container.decode(CircleJSON.self) {
            self = .circle(centerLat: circle.center_lat, centerLon: circle.center_lon, radiusMeters: circle.radius_meters)
        } else {
            let polygon = try container.decode(PolygonJSON.self)
            self = .polygon(points: polygon.points)
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .circle(let lat, let lon, let radius):
            try container.encode(CircleJSON(center_lat: lat, center_lon: lon, radius_meters: radius))
        case .polygon(let points):
            try container.encode(PolygonJSON(points: points))
        }
    }
}
