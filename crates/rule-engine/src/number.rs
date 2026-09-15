use std::cmp::Ordering;

/// Numeric observations retain the type declared by their blueprint.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum MetricNumber {
    Integer(i64),
    Float(f64),
}

impl From<f64> for MetricNumber {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl MetricNumber {
    pub fn is_finite(self) -> bool {
        match self {
            Self::Integer(_) => true,
            Self::Float(value) => value.is_finite(),
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        if let Ok(value) = value.parse::<i64>() {
            return Some(Self::Integer(value));
        }
        let value = value.parse::<f64>().ok()?;
        value.is_finite().then_some(Self::Float(value))
    }

    pub fn compare(self, other: Self) -> Option<Ordering> {
        if !self.is_finite() || !other.is_finite() {
            return None;
        }
        Some(match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => left.cmp(&right),
            (Self::Float(left), Self::Float(right)) => left.partial_cmp(&right)?,
            (Self::Integer(left), Self::Float(right)) => compare_integer_float(left, right),
            (Self::Float(left), Self::Integer(right)) => {
                compare_integer_float(right, left).reverse()
            }
        })
    }
}

fn compare_integer_float(integer: i64, float: f64) -> Ordering {
    // Bounds are exactly representable powers of two. Do not cast the integer
    // to f64: adjacent counters above 2^53 would become indistinguishable.
    if float >= 9_223_372_036_854_775_808.0 {
        return Ordering::Less;
    }
    if float < -9_223_372_036_854_775_808.0 {
        return Ordering::Greater;
    }
    match integer.cmp(&(float as i64)) {
        Ordering::Equal if float.fract() > 0.0 => Ordering::Less,
        Ordering::Equal if float.fract() < 0.0 => Ordering::Greater,
        result => result,
    }
}

impl std::fmt::Display for MetricNumber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer(value) => write!(formatter, "{value}"),
            Self::Float(value) => formatter.write_str(&crate::evaluate::format_value(*value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_adjacent_large_counters_and_boundaries() {
        let counter = MetricNumber::Integer(9_007_199_254_740_993);
        assert_eq!(
            counter.compare(MetricNumber::Integer(9_007_199_254_740_992)),
            Some(Ordering::Greater)
        );
        assert_eq!(
            counter.compare(MetricNumber::Float(9_007_199_254_740_992.0)),
            Some(Ordering::Greater)
        );
        assert_eq!(counter.to_string(), "9007199254740993");
        assert_eq!(serde_json::to_string(&counter).unwrap(), "9007199254740993");
        assert_eq!(
            MetricNumber::Integer(i64::MAX)
                .compare(MetricNumber::Float(9_223_372_036_854_775_808.0)),
            Some(Ordering::Less)
        );
        assert_eq!(
            MetricNumber::Integer(i64::MIN)
                .compare(MetricNumber::Float(-9_223_372_036_854_775_808.0)),
            Some(Ordering::Equal)
        );
        assert_eq!(
            MetricNumber::Integer(-1).compare(MetricNumber::Float(-1.5)),
            Some(Ordering::Greater)
        );
        assert_eq!(
            MetricNumber::Integer(1).compare(MetricNumber::Float(1.5)),
            Some(Ordering::Less)
        );
        assert_eq!(counter.compare(MetricNumber::Float(f64::NAN)), None);
    }
}
