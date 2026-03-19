use std::f64::consts::PI;

const EARTH_RADIUS_METERS: f64 = 6_371_000.0;

fn to_radians(degrees: f64) -> f64 {
    degrees * PI / 180.0
}

/// Haversine distance between two points in meters.
pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let d_lat = to_radians(lat2 - lat1);
    let d_lon = to_radians(lon2 - lon1);
    let lat1_rad = to_radians(lat1);
    let lat2_rad = to_radians(lat2);

    let a = (d_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    EARTH_RADIUS_METERS * c
}

/// Check if a point is within a circle defined by center + radius in meters.
pub fn point_in_circle(
    lat: f64,
    lon: f64,
    center_lat: f64,
    center_lon: f64,
    radius_meters: f64,
) -> bool {
    haversine_distance(lat, lon, center_lat, center_lon) <= radius_meters
}

/// Ray-casting algorithm: check if a point is inside a polygon.
/// Polygon defined as slice of (lat, lon) tuples. The polygon is implicitly closed.
pub fn point_in_polygon(lat: f64, lon: f64, polygon: &[(f64, f64)]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (lati, loni) = polygon[i];
        let (latj, lonj) = polygon[j];
        if ((lati > lat) != (latj > lat))
            && (lon < (lonj - loni) * (lat - lati) / (latj - lati) + loni)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_haversine_same_point() {
        let d = haversine_distance(52.2297, 21.0122, 52.2297, 21.0122);
        assert!((d - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_haversine_known_distance() {
        // Warsaw to Krakow ~252 km
        let d = haversine_distance(52.2297, 21.0122, 50.0647, 19.9450);
        assert!((d - 252_000.0).abs() < 5_000.0);
    }

    #[test]
    fn test_point_in_circle_inside() {
        assert!(point_in_circle(52.2300, 21.0125, 52.2297, 21.0122, 500.0));
    }

    #[test]
    fn test_point_in_circle_outside() {
        assert!(!point_in_circle(52.24, 21.02, 52.2297, 21.0122, 500.0));
    }

    #[test]
    fn test_point_in_polygon_inside() {
        let polygon = vec![
            (52.0, 20.0),
            (52.0, 22.0),
            (53.0, 22.0),
            (53.0, 20.0),
        ];
        assert!(point_in_polygon(52.5, 21.0, &polygon));
    }

    #[test]
    fn test_point_in_polygon_outside() {
        let polygon = vec![
            (52.0, 20.0),
            (52.0, 22.0),
            (53.0, 22.0),
            (53.0, 20.0),
        ];
        assert!(!point_in_polygon(54.0, 21.0, &polygon));
    }

    #[test]
    fn test_point_in_polygon_degenerate() {
        assert!(!point_in_polygon(0.0, 0.0, &[(1.0, 1.0), (2.0, 2.0)]));
    }
}
