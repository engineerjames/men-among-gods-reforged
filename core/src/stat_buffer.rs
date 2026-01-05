use crate::circular_buffer::CircularBuffer;

pub struct Statistics {
    pub std: f32,
    pub mean: f32,
    pub min: f32,
    pub max: f32,
}

/// Wrapper around a circular buffer that keeps track of basic statistics:
/// standard deviation, mean, min, and max. The type T must support
/// comparison and conversion to f64 for statistical calculations.
pub struct StatisticsBuffer<T: std::cmp::PartialOrd<f32> + std::convert::Into<f32> + Clone> {
    buffer: CircularBuffer<T>,
    stats: Statistics,
}

impl<T: std::cmp::PartialOrd<f32> + std::convert::Into<f32> + Clone> StatisticsBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        StatisticsBuffer {
            buffer: CircularBuffer::new(capacity),
            stats: Statistics {
                std: 0.0,
                mean: 0.0,
                min: f32::MAX,
                max: f32::MIN,
            },
        }
    }

    pub fn push(&mut self, item: T) {
        // push the new item into the buffer first so stats include it
        self.buffer.push(item.clone());

        // Collect buffer contents by using get(index) to avoid requiring an iter() method
        let mut data_as_f32: Vec<f32> = Vec::new();
        let mut idx = 0;
        while let Some(elem) = self.buffer.get(idx) {
            data_as_f32.push(elem.clone().into());
            idx += 1;
        }

        // Update min and max based on collected data
        if !data_as_f32.is_empty() {
            self.stats.min = data_as_f32.iter().cloned().fold(f32::MAX, f32::min);
            self.stats.max = data_as_f32.iter().cloned().fold(f32::MIN, f32::max);
        } else {
            self.stats.min = f32::MAX;
            self.stats.max = f32::MIN;
        }

        // Update mean and std (simple approach)
        self.stats.mean = Self::mean(&data_as_f32).unwrap_or(0.0);
        self.stats.std = Self::std_deviation(&data_as_f32).unwrap_or(0.0);
    }

    fn mean(data: &[f32]) -> Option<f32> {
        let sum = data.iter().sum::<f32>();
        let count = data.len();

        match count {
            positive if positive > 0 => Some(sum / count as f32),
            _ => None,
        }
    }

    fn std_deviation(data: &[f32]) -> Option<f32> {
        match (Self::mean(data), data.len()) {
            (Some(data_mean), count) if count > 0 => {
                let variance = data
                    .iter()
                    .map(|value| {
                        let diff = data_mean - (*value);

                        diff * diff
                    })
                    .sum::<f32>()
                    / count as f32;

                Some(variance.sqrt())
            }
            _ => None,
        }
    }

    pub fn stats(&self) -> &Statistics {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.stats.std = 0.0;
        self.stats.mean = 0.0;
        self.stats.min = f32::MAX;
        self.stats.max = f32::MIN;
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.buffer.get(index)
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::StatisticsBuffer;

    fn assert_close(actual: f32, expected: f32) {
        let eps = 1e-6_f32;
        assert!(
            (actual - expected).abs() <= eps,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn new_initializes_empty_stats() {
        let buf = StatisticsBuffer::<f32>::new(3);
        assert_eq!(buf.len(), 0);

        let stats = buf.stats();
        assert_close(stats.mean, 0.0);
        assert_close(stats.std, 0.0);
        assert_eq!(stats.min, f32::MAX);
        assert_eq!(stats.max, f32::MIN);
    }

    #[test]
    fn push_single_value_updates_stats() {
        let mut buf = StatisticsBuffer::<f32>::new(3);
        buf.push(5.0);

        assert_eq!(buf.len(), 1);
        assert_eq!(buf.get(0), Some(&5.0));

        let stats = buf.stats();
        assert_close(stats.mean, 5.0);
        assert_close(stats.std, 0.0);
        assert_close(stats.min, 5.0);
        assert_close(stats.max, 5.0);
    }

    #[test]
    fn push_multiple_values_computes_mean_min_max_and_population_std() {
        let mut buf = StatisticsBuffer::<f32>::new(5);
        buf.push(1.0);
        buf.push(2.0);
        buf.push(3.0);

        assert_eq!(buf.len(), 3);
        let stats = buf.stats();

        assert_close(stats.mean, 2.0);
        assert_close(stats.min, 1.0);
        assert_close(stats.max, 3.0);

        // population variance = ((1-2)^2 + (2-2)^2 + (3-2)^2) / 3 = 2/3
        let expected_std = (2.0_f32 / 3.0).sqrt();
        assert_close(stats.std, expected_std);
    }

    #[test]
    fn overwrite_behavior_stats_only_include_current_buffer_contents() {
        let mut buf = StatisticsBuffer::<f32>::new(3);
        buf.push(1.0);
        buf.push(2.0);
        buf.push(3.0);
        buf.push(4.0); // overwrites 1.0

        assert_eq!(buf.len(), 3);
        assert_eq!(buf.get(0), Some(&4.0));
        assert_eq!(buf.get(1), Some(&3.0));
        assert_eq!(buf.get(2), Some(&2.0));

        let stats = buf.stats();
        assert_close(stats.min, 2.0);
        assert_close(stats.max, 4.0);
        assert_close(stats.mean, 3.0);

        // data is [4,3,2], mean 3, variance 2/3
        let expected_std = (2.0_f32 / 3.0).sqrt();
        assert_close(stats.std, expected_std);
    }

    #[test]
    fn clear_resets_stats_and_buffer() {
        let mut buf = StatisticsBuffer::<f32>::new(3);
        buf.push(10.0);
        buf.push(20.0);
        assert_eq!(buf.len(), 2);

        buf.clear();
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.get(0), None);

        let stats = buf.stats();
        assert_close(stats.mean, 0.0);
        assert_close(stats.std, 0.0);
        assert_eq!(stats.min, f32::MAX);
        assert_eq!(stats.max, f32::MIN);
    }
}
