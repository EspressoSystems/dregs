use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PartitionError {
    #[error("invalid partition format: {0} (expected 'slice:M/N')")]
    InvalidFormat(String),
    #[error("partition index must be >= 1, got {0}")]
    IndexTooLow(u32),
    #[error("partition index {index} exceeds total {total}")]
    IndexExceedsTotal { index: u32, total: u32 },
    #[error("partition total must be >= 1, got {0}")]
    TotalTooLow(u32),
}

pub type Result<T> = std::result::Result<T, PartitionError>;

#[derive(Debug, Clone, PartialEq)]
pub struct Partition {
    pub index: u32, // 1-based
    pub total: u32,
}

impl Partition {
    /// Filter items by round-robin assignment: item at position i (0-based)
    /// belongs to partition (i % total) + 1
    pub fn filter<'a, T, F>(&self, items: &'a [T], id_fn: F) -> Vec<&'a T>
    where
        F: Fn(&T) -> u32,
    {
        items
            .iter()
            .filter(|item| (id_fn(item) - 1) % self.total == self.index - 1)
            .collect()
    }
}

impl FromStr for Partition {
    type Err = PartitionError;

    fn from_str(s: &str) -> Result<Self> {
        let rest = s
            .strip_prefix("slice:")
            .ok_or_else(|| PartitionError::InvalidFormat(s.to_string()))?;

        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() != 2 {
            return Err(PartitionError::InvalidFormat(s.to_string()));
        }

        let index: u32 = parts[0]
            .parse()
            .map_err(|_| PartitionError::InvalidFormat(s.to_string()))?;
        let total: u32 = parts[1]
            .parse()
            .map_err(|_| PartitionError::InvalidFormat(s.to_string()))?;

        if total == 0 {
            return Err(PartitionError::TotalTooLow(total));
        }
        if index == 0 {
            return Err(PartitionError::IndexTooLow(index));
        }
        if index > total {
            return Err(PartitionError::IndexExceedsTotal { index, total });
        }

        Ok(Partition { index, total })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ok() {
        let p: Partition = "slice:2/4".parse().unwrap();
        assert_eq!(p, Partition { index: 2, total: 4 });
    }

    #[test]
    fn parse_invalid_fails() {
        assert!("slice:0/4".parse::<Partition>().is_err());
        assert!("slice:5/4".parse::<Partition>().is_err());
        assert!("bad".parse::<Partition>().is_err());
        assert!("hash:1/2".parse::<Partition>().is_err());
        assert!("slice:1/0".parse::<Partition>().is_err());
        assert!("slice:abc/2".parse::<Partition>().is_err());
        assert!("slice:1/2/3".parse::<Partition>().is_err());
    }

    #[test]
    fn filter_ok() {
        let items: Vec<u32> = (1..=10).collect();
        let p: Partition = "slice:1/3".parse().unwrap();
        let filtered: Vec<u32> = p.filter(&items, |x| *x).into_iter().copied().collect();
        assert_eq!(filtered, vec![1, 4, 7, 10]);
    }

    #[test]
    fn single_shard_ok() {
        let items: Vec<u32> = (1..=5).collect();
        let p: Partition = "slice:1/1".parse().unwrap();
        let filtered: Vec<u32> = p.filter(&items, |x| *x).into_iter().copied().collect();
        assert_eq!(filtered, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn empty_shard_ok() {
        let items: Vec<u32> = vec![1, 2, 3];
        let p: Partition = "slice:4/5".parse().unwrap();
        let filtered: Vec<u32> = p.filter(&items, |x| *x).into_iter().copied().collect();
        assert!(filtered.is_empty());
    }

    #[test]
    fn equal_distribution_ok() {
        let items: Vec<u32> = (1..=6).collect();
        let mut all: Vec<u32> = Vec::new();
        for i in 1..=3 {
            let p = Partition { index: i, total: 3 };
            let filtered: Vec<u32> = p.filter(&items, |x| *x).into_iter().copied().collect();
            assert_eq!(filtered.len(), 2);
            all.extend(filtered);
        }
        all.sort();
        assert_eq!(all, vec![1, 2, 3, 4, 5, 6]);
    }
}
