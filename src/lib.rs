use std::convert::Infallible;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub struct MetricFamily {
    pub name: String,
    pub metrics: Vec<Metric>,
}

#[derive(Debug, PartialEq)]
pub enum Metric {
    Counter(Vec<MetricLabel>, Option<f64>),
    Gauge(Vec<MetricLabel>, Option<f64>),
    Histogram {
        labels: Vec<MetricLabel>,
        sample_count: Option<u64>,
        sample_sum: Option<f64>,
    },
    UNSUPPORTED(prometheus::proto::MetricType),
}

impl Metric {
    pub fn count(&self) -> u64 {
        match self {
            Self::Counter(_, _) => 1,
            Self::Gauge(_, _) => 1,
            Self::Histogram { sample_count, .. } => (*sample_count).unwrap_or(0),
            Self::UNSUPPORTED(_) => 1,
        }
    }

    pub fn sum(&self) -> f64 {
        match self {
            Self::Counter(_, val) => (*val).unwrap_or(0_f64),
            Self::Gauge(_, val) => (*val).unwrap_or(0_f64),
            Self::Histogram { sample_sum, .. } => (*sample_sum).unwrap_or(0_f64),
            Self::UNSUPPORTED(_) => 0_f64,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MetricLabel {
    pub name: String,
    pub value: String,
}

impl From<&str> for MetricLabel {
    fn from(rep: &str) -> Self {
        MetricLabel::from_str(rep).unwrap()
    }
}

impl From<String> for MetricLabel {
    fn from(rep: String) -> Self {
        MetricLabel::from_str(rep.as_str()).unwrap()
    }
}

impl FromStr for MetricLabel {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.split_once("|")
            .map(|(name, value)| Ok(Self { name: name.to_string(), value: value.to_string() }))
            .unwrap_or_else(|| Ok(Self { name: String::new(), value: String::new() }))
    }
}

pub fn distill_metric_state(families: impl IntoIterator<Item = prometheus::proto::MetricFamily>) -> Vec<MetricFamily> {
    families
        .into_iter()
        .map(|family| {
            let name = family.get_name().to_string();
            let metrics: Vec<Metric> = family
                .get_metric()
                .iter()
                .cloned()
                .map(|m| {
                    let labels: Vec<MetricLabel> = m
                        .get_label()
                        .iter()
                        .map(|l| MetricLabel {
                            name: l.get_name().to_string(),
                            value: l.get_value().to_string(),
                        })
                        .collect();

                    match family.get_field_type() {
                        prometheus::proto::MetricType::COUNTER => {
                            let c = m.get_counter();
                            let val = if c.has_value() { Some(c.get_value()) } else { None };
                            Metric::Counter(labels, val)
                        },
                        prometheus::proto::MetricType::GAUGE => {
                            let g = m.get_gauge();
                            let val = if g.has_value() { Some(g.get_value()) } else { None };
                            Metric::Gauge(labels, val)
                        },
                        prometheus::proto::MetricType::HISTOGRAM => {
                            let h = m.get_histogram();
                            let sample_count = if h.has_sample_count() { Some(h.get_sample_count()) } else { None };
                            let sample_sum = if h.has_sample_sum() { Some(h.get_sample_sum()) } else { None };
                            Metric::Histogram { labels, sample_count, sample_sum }
                        },
                        metric_type => {
                            tracing::error!("prometheus::proto metric_type not supported: {:?}", metric_type);
                            Metric::UNSUPPORTED(metric_type)
                        },
                    }
                })
                .collect();

            MetricFamily { name, metrics }
        })
        .collect()
}
