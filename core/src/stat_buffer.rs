use crate::circular_buffer::CircularBuffer;

pub struct Statistics {
    std: f32,
    mean: f32,
    min: f32,
    max: f32,
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
}
